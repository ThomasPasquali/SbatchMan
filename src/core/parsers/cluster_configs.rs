use std::path::Path;

use log::debug;

use crate::core::{database::models::NewClusterConfig, parsers::{includes::get_include_variables, utils::{load_yaml_from_file, lookup_sequence}, ParserError}};

fn parse_cluster<'a, 'b>(yaml: &'a saphyr::YamlOwned) -> Result<Vec<NewClusterConfig<'b>>, ParserError> {
  // TODO: implement parsing logic here
  Ok(vec![])
}

pub fn parse_clusters_configs_from_file(
  root: &Path,
) -> Result<Vec<NewClusterConfig<'_>>, ParserError> {
  let variables = get_include_variables(root)?;
  debug!("Parsed variables: {:?}", variables);
  let yaml = load_yaml_from_file(root)?;

  let clusters = lookup_sequence(&yaml, "clusters").map_err(|_| ParserError::EmptyClusterConfig)?;
  let mut configs = vec![];
  for cluster in clusters {
    let mut to_append = parse_cluster(cluster)?;
    configs.append(&mut to_append);
  }
  Ok(configs)
}