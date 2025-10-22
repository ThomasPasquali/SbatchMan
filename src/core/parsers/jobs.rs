use std::path::PathBuf;

use crate::core::{parsers::ParserError};

pub struct ParsedJob<'a> {
  pub job_name: &'a str,
  pub config_name: &'a str,
  pub command: &'a str,
  pub preprocess: Option<&'a str>,
  pub postprocess: Option<&'a str>,
  pub variables: &'a serde_json::Value,
}

pub fn parse_jobs_from_file(path: &PathBuf) -> Result<Vec<ParsedJob<'_>>, ParserError> {
  // FIXME implement job parsing logic
  Ok(vec![])
}