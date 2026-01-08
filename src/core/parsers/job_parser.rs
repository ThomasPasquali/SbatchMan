use std::{
  path::PathBuf,
};

use crate::core::{parsers::{ParserError, includes::parse_include_variables, multi_hashmap::MultiHashMap, yaml_parser::{check_mapping_keys, load_yaml_from_file, lookup_mapping}}};

pub struct ParsedJob<'a> {
  pub job_name: &'a str,
  pub config_name: &'a str,
  pub command: &'a str,
  pub preprocess: Option<&'a str>,
  pub postprocess: Option<&'a str>,
  pub variables: &'a serde_json::Value,
}

pub fn parse_jobs_from_file(root: &PathBuf) -> Result<Vec<ParsedJob<'_>>, ParserError> {
  let yaml = load_yaml_from_file(root)?;
  let required_keys = vec!["jobs"];
  let optional_keys = vec!["command", "preprocess", "postprocess", "include", "variables"];
  check_mapping_keys(&yaml, &required_keys, &optional_keys)?;

  let mut variables = MultiHashMap::new();
  parse_include_variables(&yaml, root, &mut variables)?;

  let mut parsed_jobs = vec![];
  /*for (job_name, configs) in lookup_mapping(&yaml, "jobs")? {
    parsed_jobs.push(parse_job(
      to_string(job_name)?,
      configs,
      &mut variables,
    )?);
  }*/
  Ok(parsed_jobs)

}
