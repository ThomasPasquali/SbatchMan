mod configs;
mod includes;
mod jobs;
mod utils;
pub mod variables;

#[cfg(test)]
mod tests;

use thiserror::Error;

pub use configs::parse_clusters_configs_from_file;
pub use jobs::{ParsedJob, parse_jobs_from_file};

#[derive(Error, Debug)]
pub enum ParserError {
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("YAML parsing failed: {0}")]
  YamlParseFailed(#[from] saphyr::ScanError),
  #[error(
    "The file {0} is being included multiple times. Check if it has been included from multiple files or if there is a circular include (ex. FILE 1 -> FILE 2 -> FILE 1)."
  )]
  CircularInclude(String),
  #[error("YAML file is empty!")]
  YamlEmpty,
  #[error("Eval Error: {0}")]
  EvalError(String),
  #[error("Missing Key: {0}")]
  MissingKey(String),
  #[error("Cluster config file is empty!")]
  EmptyClusterConfig,
  #[error("Wrong type for value \"{0}\", expected type {1}")]
  WrongType(String, String),
  #[error("Include error: {0} is neither a string nor a sequence")]
  IncludeWrongType(String),
}

/*pub fn parse_clusters_configs_from_file(
  root: &Path,
) -> Result<Vec<NewClusterConfig<'_>>, ParserError> {
  let variables = get_include_variables(root)?;

  debug!("Parsed variables: {:?}", variables);
  Ok(vec![])

  // FIXME
  // panic!("Not tested from here on!!!");
  /*
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

  Ok(fully_expanded_clusters)*/
}
*/
