use std::{
  collections::HashMap, path::PathBuf
};

use saphyr::YamlOwned;

use crate::core::parsers::{ParserError, combination_generator::CombinationGenerator, includes::parse_include_variables, multi_hashmap::MultiHashMap, template_parser::Template, variable_parser::{CompleteVar, parse_variables}, yaml_parser::{check_mapping_keys, load_yaml_from_file, lookup_sequence, lookup_str, to_sequence, yaml_lookup}};

pub struct ParsedJob {
  pub job_name: String,
  pub config_name: String,
  pub command: String,
  pub preprocess: Option<String>,
  pub postprocess: Option<String>,
}

fn parse_job_config(yaml: &YamlOwned, config: &mut MultiHashMap<String, Template>) -> Result<(), ParserError> {
  let mut config_map = HashMap::new();
  if let Ok(value) = lookup_str(yaml, "name") {
    config_map.insert("name".to_string(), Template::from_str(&value)?);
  }
  if let Ok(value) = lookup_str(yaml, "cluster_config") {
    config_map.insert("cluster_config".to_string(), Template::from_str(&value)?);
  }
  if let Ok(value) = lookup_str(yaml, "command") {
    config_map.insert("command".to_string(), Template::from_str(&value)?);
  }
  if let Ok(value) = lookup_str(yaml, "preprocess") {
    config_map.insert("preprocess".to_string(), Template::from_str(&value)?);
  }
  if let Ok(value) = lookup_str(yaml, "postprocess") {
    config_map.insert("postprocess".to_string(), Template::from_str(&value)?);
  }
  if let Ok(value) = lookup_str(yaml, "command") {
    config_map.insert("command".to_string(), Template::from_str(&value)?);
  }

  config.push(config_map);

  Ok(())
}

fn generate_jobs(variables: &MultiHashMap<String, CompleteVar>, config: &MultiHashMap<String, Template>, cluster_name: &str) -> Result<Vec<ParsedJob>, ParserError> {
  let mut generator = CombinationGenerator::new(variables, &cluster_name);
  for (_, template) in config.iter() {
    generator.register_template(template)?;
  }

  let mut jobs = vec![];
  let mut combinations = generator.try_iter()?;

  while combinations.next().is_some() {
    let job = ParsedJob {
      job_name: config.get("name")
        .ok_or(ParserError::MissingKey("name".to_string()))?
        .render(&combinations)?,
      config_name: config.get("cluster_config")
        .ok_or(ParserError::MissingKey("cluster_config".to_string()))?
        .render(&combinations)?,
      command: config.get("command")
        .ok_or(ParserError::MissingKey("command".to_string()))?
        .render(&combinations)?,
      preprocess: config.get("preprocess").map(|t| t.render(&combinations)).transpose()?,
      postprocess: config.get("postprocess").map(|t| t.render(&combinations)).transpose()?,
    };
    jobs.push(job);
  }

  Ok(jobs)
}

fn parse_variant(yaml: &YamlOwned, variables: &mut MultiHashMap<String, CompleteVar>, config: &mut MultiHashMap<String, Template>, cluster_name: &str, path: &PathBuf) -> Result<Vec<ParsedJob>, ParserError> {
  let required_keys = vec!["name"];
  let optional_keys = vec!["command", "preprocess", "postprocess", "variables"];
  check_mapping_keys(&yaml, &required_keys, &optional_keys)?;

  parse_variables(yaml, variables, path)?;

  parse_job_config(yaml, config)?;

  let jobs = generate_jobs(variables, config, cluster_name)?;

  variables.pop();
  config.pop();
  
  Ok(jobs)
}

fn parse_job(
  yaml: &YamlOwned,
  variables: &mut MultiHashMap<String, CompleteVar>,
  config: &mut MultiHashMap<String, Template>,
  cluster_name: &str,
  path: &PathBuf,
) -> Result<Vec<ParsedJob>, ParserError> {
  let required_keys = vec!["name", "cluster_config"];
  let optional_keys = vec!["command", "preprocess", "postprocess", "variables", "variants"];
  check_mapping_keys(&yaml, &required_keys, &optional_keys)?;

  parse_variables(yaml, variables, path)?;

  parse_job_config(yaml, config)?;

  let mut jobs = vec![];
  
  if let Some(variants) = yaml_lookup(yaml, "variants") {
    for variant in to_sequence(&variants)? {
      let mut variant_jobs = parse_variant(variant, variables, config, cluster_name, path)?;
      jobs.append(&mut variant_jobs);
    }
  } else {
    jobs.append(&mut generate_jobs(variables, config, cluster_name)?);
  }

  variables.pop();
  config.pop();

  Ok(jobs)
}

pub fn parse_jobs_from_file(root: &PathBuf, cluster_name: &str) -> Result<Vec<ParsedJob>, ParserError> {
  let yaml = load_yaml_from_file(root)?;
  let required_keys = vec!["jobs"];
  let optional_keys = vec!["command", "preprocess", "postprocess", "include", "variables"];
  check_mapping_keys(&yaml, &required_keys, &optional_keys)?;

  let mut variables = MultiHashMap::new();
  parse_include_variables(&yaml, root, &mut variables)?;

  let mut config = MultiHashMap::new();
  parse_job_config(&yaml, &mut config)?;

  let mut parsed_jobs = vec![];
  for job_yaml in lookup_sequence(&yaml, "jobs")? {
    parsed_jobs.append(&mut parse_job(
      job_yaml,
      &mut variables,
      &mut config,
      cluster_name,
      root,
    )?);
  }
  Ok(parsed_jobs)
}
