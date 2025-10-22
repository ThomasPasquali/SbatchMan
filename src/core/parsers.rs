mod jobs;
mod utils;
mod variables;

#[cfg(test)]
mod tests;

use hashlink::LinkedHashMap;
use log::{debug};
use saphyr::{LoadableYamlNode, Yaml};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;

use crate::core::database::models::{NewCluster, NewClusterConfig, NewConfig, Scheduler};
use crate::core::parsers::utils::{
  lookup_sequence, lookup_str, yaml_lookup};
use crate::core::parsers::variables::{Variable, parse_variables};
pub use jobs::{ParsedJob, parse_jobs_from_file};

#[derive(Error, Debug)]
pub enum ParserError {
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("YAML parsing failed: {0}")]
  YamlParseFailed(#[from] saphyr::ScanError),
  #[error("YAML file is empty!")]
  YamlEmpty,
  #[error("Eval Error: {0}")]
  EvalError(String),
  #[error("Missing Key: {0}")]
  MissingKey(String),
  #[error("Wrong type for value \"{0}\", expected type {1}")]
  WrongType(String, String),
}

/// Intermediate representation of a parsed configuration (before mapping to DB models)
#[derive(Debug, Clone)]
pub struct ParsedConfig {
  pub name_template: String,
  pub params: serde_json::Value,
  pub extra_headers: Vec<String>,
}

fn get_include_variables<'a>(yaml: &Yaml, path: &Path) -> Result<LinkedHashMap<String, Variable>, ParserError> {
  debug!("Loading included variables from file: {:?}", &path);
  let mut variables = LinkedHashMap::new();

  if let Some(yaml_variables) = yaml_lookup(&yaml, "variables") {
    let new_variables = parse_variables(&yaml_variables)?;
    variables.extend(new_variables);
  }

  let mut include_files: Vec<String> = Vec::new();

  // Check for single include or sequence of includes
  if let Ok(include_file) = lookup_str(&yaml, "include") {
    include_files.push(include_file);
  } else if let Ok(include_sequence) = lookup_sequence(&yaml, "include") {
    for it in include_sequence.iter() {
      if let Some(s) = it.as_str() {
        include_files.push(s.to_string());
      }
    }
  }

  for file in include_files {
    // If path is absolute, use it directly; otherwise, join with parent directory
    let path = if Path::new(&file).is_absolute() {
      PathBuf::from(&file)
    } else {
      // Throw error if parent is None
      path.parent().ok_or(ParserError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("Cannot determine parent directory of path {:?}", path),
      )))?.join(&file)
    };
    let text = fs::read_to_string(&path)?;
    let yaml = Yaml::load_from_str(&text)
      .map_err(ParserError::YamlParseFailed)?
      .into_iter()
      .next()
      .ok_or(ParserError::YamlEmpty)?;
    let inc_vars = get_include_variables(&yaml, &path)?;
    variables.extend(inc_vars);
  }

  Ok(variables)
}

/// Public parser entry. Returns intermediate ParsedCluster objects.
pub fn parse_clusters_configs_from_file(root: &Path) -> Result<Vec<NewClusterConfig>, ParserError> {
  let text = fs::read_to_string(&root)?;
  let yaml = Yaml::load_from_str(&text)
    .map_err(ParserError::YamlParseFailed)?
    .into_iter()
    .next()
    .ok_or(ParserError::YamlEmpty)?;
  
  let variables = get_include_variables(&yaml, root)?;

  // FIXME
  panic!("Not tested from here on!!!");

  // Top-level variables collection
  let variables_map = collect_variables(&yaml, &base_dir)?;

  // default_params top-level
  let default_params_node =
    yaml_lookup(&yaml, "default_params").unwrap_or(&Yaml::Value(saphyr::Scalar::Null));
  let default_params = yamlnode_to_json(&default_params_node)?;

  // clusters
  let clusters_node = yaml_lookup(&yaml, "clusters")
    .ok_or_else(|| ParserError::YamlParseError("Missing top-level 'clusters' key".to_string()))?;

  if !clusters_node.is_mapping() {
    return Err(ParserError::YamlParseError(
      "'clusters' must be a mapping".to_string(),
    ));
  }

  let mut parsed_clusters = Vec::new();

  if let Yaml::Mapping(clusters_map) = &clusters_node {
    for (cluster_key_node, cluster_val_node) in clusters_map.iter() {
      let cluster_name = cluster_key_node.as_str().unwrap_or("").to_string();
      if cluster_name.is_empty() {
        return Err(ParserError::YamlParseError(
          "Cluster name must be a string".to_string(),
        ));
      }

      // Each cluster can have its own variables
      let mut combined_vars = variables_map.clone();

      if let Some(cluster_vars_node) = yaml_lookup(cluster_val_node, "variables") {
        let cluster_vars = parse_variables_block(&cluster_vars_node, &base_dir)?;
        combined_vars.extend(cluster_vars);
      }

      // Cluster-level default_params overriding top-level
      let mut cluster_default_params = default_params.clone();
      if let Some(cluster_dp) = yaml_lookup(cluster_val_node, "default_params") {
        let jp = yamlnode_to_json(&cluster_dp)?;
        merge_json(&mut cluster_default_params, &jp);
      }

      // scheduler
      let scheduler = yaml_lookup(cluster_val_node, "scheduler")
        .and_then(|n| n.as_str())
        .and_then(|s| Scheduler::from_str(s).ok())
        .ok_or_else(|| {
          ParserError::YamlParseError(format!(
            "Cluster '{}' has invalid or missing scheduler",
            cluster_name
          ))
        })?;

      // max_jobs
      let max_jobs = yaml_lookup(cluster_val_node, "max_jobs")
        .and_then(|n| n.as_integer())
        .map(|i| i as i32);

      // configs
      let mut parsed_configs = Vec::new();

      if let Some(cfgs) = yaml_lookup(cluster_val_node, "configs") {
        if !cfgs.is_sequence() {
          return Err(ParserError::YamlParseError(
            "configs should be a sequence".to_string(),
          ));
        }
        if let Yaml::Sequence(cfg_seq) = &cfgs {
          for cfg_item in cfg_seq.iter() {
            // name (required)
            let name_templ = yaml_lookup(cfg_item, "name")
              .and_then(|n| Some(String::from_str(n.as_str().unwrap()).unwrap()))
              .ok_or_else(|| {
                ParserError::YamlParseError("Each config needs a 'name' string".to_string())
              })?;

            // params override for this config
            let mut cfg_params = cluster_default_params.clone();
            if let Some(p) = yaml_lookup(cfg_item, "params") {
              let jp = yamlnode_to_json(&p)?;
              merge_json(&mut cfg_params, &jp);
            }

            // extra_headers
            let mut extra_headers = Vec::new();
            if let Some(headers_node) = yaml_lookup(cfg_item, "extra_headers") {
              if let Yaml::Sequence(header_seq) = &headers_node {
                for h in header_seq.iter() {
                  if let Some(s) = h.as_str() {
                    extra_headers.push(s.to_string());
                  }
                }
              }
            }

            parsed_configs.push(NewConfig {
              config_name: &name_templ,
              flags: None,
              env: None,
              // FIXME
            });
          }
        }
      }

      parsed_clusters.push(NewClusterConfig {
        cluster: NewCluster {
          cluster_name: &cluster_name,
          scheduler,
          max_jobs,
        },
        configs: parsed_configs,
      });
    }
  }

  // Expand variables within configs and default_params
  let mut fully_expanded_clusters = Vec::new();

  for mut pc in parsed_clusters {
    // Expand default_params
    let expanded_dp = expand_json_templates(&pc.default_params, &variables_map, None)?;
    pc.default_params = expanded_dp;

    // Expand configs
    let mut expanded_configs = Vec::new();
    for cfg in pc.configs {
      // Expand name template
      let name_expanded = expand_string_templates(&cfg.name_template, &variables_map, None)?;

      // Expand params object recursively
      let params_expanded = expand_json_templates(&cfg.params, &variables_map, None)?;

      // Expand extra headers
      let mut ex_headers_expanded = Vec::new();
      for h in cfg.extra_headers {
        let s = expand_string_templates(&h, &variables_map, None)?;
        ex_headers_expanded.push(s);
      }

      expanded_configs.push(ParsedConfig {
        name_template: name_expanded,
        params: params_expanded,
        extra_headers: ex_headers_expanded,
      });
    }
    pc.configs = expanded_configs;

    fully_expanded_clusters.push(pc);
  }

  Ok(fully_expanded_clusters)
}
