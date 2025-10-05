use thiserror::Error;

use crate::core::storage::models::{NewCluster, NewConfig};

pub struct NewClusterConfig<'a> {
  pub cluster: NewCluster<'a>,
  pub configs: Vec<NewConfig<'a>>,
}

#[derive(Error, Debug)]
pub enum ParserError {
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("YAML Parse Error: {0}")]
  YamlParseError(String),
}

pub fn parse_clusters_configs_from_file(path: &str) -> Result<Vec<NewClusterConfig>, ParserError> {
  let f = std::fs::read_to_string(path)?;

  // TODO
  // A general flow for parsing the clusters configs file could be:
  // 1. Parse the initial YAML file
  // 2. Find the "includes" section and append the included files
  // 3. Repeat steps 1-2 until no more includes are found
  // 4. Parse the combined YAML
  // 5. Find the top-level variables section and store the variables
  // 6. Iterate over the whole structure, replace variables and run the python snippets
  // 7. Convert the final structure into a NewClusterConfig list
  // Note: Leave the cluster_id field to 0, it will be set when inserting into the DB
  Ok(vec![])
}