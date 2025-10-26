use std::path::Path;

use log::debug;

use crate::core::{
  database::{
    models::{NewCluster, NewClusterConfig},
    schema::configs,
  },
  parsers::{
    ParserError,
    includes::get_include_variables,
    utils::{load_yaml_from_file, lookup_mapping, to_sequence, to_string},
  },
};

fn parse_cluster_configs<'a, 'b>(
  cluster_name: &'a saphyr::YamlOwned,
  configs: &'a saphyr::YamlOwned,
) -> Result<Vec<NewClusterConfig<'b>>, ParserError> {
  /*let cluster_name = to_string(&cluster_name)?;
  let parsed_cluster = NewCluster {
    cluster_name: &cluster_name,
    scheduler:

  };
  let configs = to_sequence(configs)?;
  for config in configs {
    let parsed_config = NewClusterConfig {

    };
  }*/
  // TODO: implement parsing logic here
  Ok(vec![])
}

pub fn parse_clusters_configs_from_file(
  root: &Path,
) -> Result<Vec<NewClusterConfig<'_>>, ParserError> {
  let variables = get_include_variables(root)?;
  debug!("Parsed variables: {:?}", variables);
  let yaml = load_yaml_from_file(root)?;

  let clusters = lookup_mapping(&yaml, "clusters").map_err(|_| ParserError::EmptyClusterConfig)?;
  let mut parsed_configs = vec![];
  for (cluster_name, configs) in clusters {
    let mut to_append = parse_cluster_configs(cluster_name, configs)?;
    parsed_configs.append(&mut to_append);
  }
  Ok(parsed_configs)
}
