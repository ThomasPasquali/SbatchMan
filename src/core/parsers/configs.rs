use std::{
  collections::{HashMap, HashSet},
  path::Path,
  str::FromStr,
};

use hashlink::LinkedHashMap;
use once_cell::sync::Lazy;
use saphyr::YamlOwned;
use serde_json::json;

use crate::core::{
  database::models::{NewCluster, NewClusterConfig, NewConfig, Scheduler},
  parsers::{
    ParserError,
    includes::get_include_variables,
    utils::{
      load_yaml_from_file, lookup_mapping, lookup_sequence, lookup_str, to_mapping, to_string,
      value_from_str, yaml_lookup,
    },
    variables::{Variable, parse_variables},
  },
};

impl Scheduler {
  const LOCAL_PARAMS: Lazy<HashSet<&str>> = Lazy::new(|| HashSet::from(["time"]));

  const PBS_PARAMS: Lazy<HashSet<&str>> =
    Lazy::new(|| HashSet::from(["queue", "cpus", "mem", "walltime"]));

  #[rustfmt::skip]
  const SLURM_PARAMS: Lazy<HashSet<&str>> = Lazy::new(|| {
    HashSet::from([
      "partition", "nodes", "ntasks", "tasks_per_node", "cpus_per_task",
      "mem", "account", "time", "gpus", "nodelist", "exclude", "qos",
      "reservation", "exclusive", "modules",
    ])
  });

  fn has_param(&self, param: &str) -> bool {
    match self {
      Scheduler::Local => Self::LOCAL_PARAMS.contains(param),
      Scheduler::Slurm => Self::SLURM_PARAMS.contains(param),
      Scheduler::Pbs => Self::PBS_PARAMS.contains(param),
    }
  }
}

#[derive(Default)]
struct Parameters {
  options: HashMap<String, String>,
  env: HashMap<String, String>,
}

// Takes as input a mapping and returns an object containing the list of options and env variables
fn parse_params(
  params_node: &LinkedHashMap<YamlOwned, YamlOwned>,
  scheduler: &Scheduler,
) -> Result<Parameters, ParserError> {
  let mut params = Parameters::default();
  if let Some(env_node) = params_node.get(&value_from_str("env")) {
    // Parse env variables
    let env_mapping = to_mapping(env_node)?;
    let mut env = HashMap::new();
    for (key_node, value_node) in env_mapping {
      let key = to_string(key_node)?;
      let value = to_string(value_node)?;
      env.insert(key, value);
    }
    params.env = env;
  }

  for (key_node, value_node) in params_node {
    let key = to_string(key_node)?;
    // Skip env as it has been already processed
    if key == "env" || key == "variables" {
      continue;
    }
    // Check if the parameter is valid for the scheduler. If not, return an error
    if !scheduler.has_param(&key) {
      return Err(ParserError::InvalidParameterForScheduler(
        key,
        format!("{:?}", scheduler),
      ));
    }
    let value = to_string(value_node)?;
    params.options.insert(key, value);
  }
  Ok(params)
}

fn parse_config(
  config: &YamlOwned,
  scheduler: &Scheduler,
  top_variables: &LinkedHashMap<String, Variable>,
  cluster_variables: &LinkedHashMap<String, Variable>,
  cluster_params: &Parameters,
) -> Result<NewConfig, ParserError> {
  // Parse variables
  let cluster_variables = match lookup_mapping(config, "variables") {
    Ok(variables) => parse_variables(variables)?,
    Err(_) => LinkedHashMap::new(),
  };

  // Parse params (options and env)
  let cluster_params = match lookup_mapping(config, "params") {
    Ok(defaults) => parse_params(defaults, &scheduler)?,
    Err(_) => Parameters::default(),
  };

  // Name
  let name = lookup_str(config, "name")?;
  // TODO: substitute variables in name

  // TODO: Generate correct flags and env by merging top_variables, cluster_variables, cluster_params, and config-specific ones
  let flags: Vec<String> = vec![];
  let env: Vec<String> = vec![];

  Ok(NewConfig {
    config_name: name,
    cluster_id: 0,
    flags: json!(flags),
    env: json!(env),
  })
}

fn parse_cluster(
  cluster_name: String,
  cluster: &saphyr::YamlOwned,
  top_variables: &LinkedHashMap<String, Variable>,
) -> Result<NewClusterConfig, ParserError> {
  // Parse scheduler
  let scheduler_str = lookup_str(cluster, "scheduler")?;
  let scheduler = Scheduler::from_str(&scheduler_str)
    .map_err(|_| ParserError::InvalidScheduler(scheduler_str.clone()))?;

  // Parse cluster-level variables
  let cluster_variables = match lookup_mapping(cluster, "variables") {
    Ok(variables) => parse_variables(variables)?,
    Err(_) => LinkedHashMap::new(),
  };

  // Parse cluster-level default params (options and env)
  let cluster_params = match lookup_mapping(cluster, "defaults") {
    Ok(defaults) => parse_params(defaults, &scheduler)?,
    Err(_) => Parameters::default(),
  };

  // Max jobs
  let max_jobs = yaml_lookup(cluster, "max_jobs")
    .and_then(|n| n.as_integer())
    .map(|i| i as i32);

  // Configs
  let mut parsed_cluster = NewClusterConfig {
    cluster: NewCluster {
      cluster_name: cluster_name,
      scheduler: scheduler.clone(),
      max_jobs,
    },
    configs: vec![],
  };

  let configs = lookup_sequence(cluster, "configs")?;
  for config in configs.iter() {
    parsed_cluster
      .configs
      .push(parse_config(config, &scheduler, top_variables, &cluster_variables, &cluster_params)?);
  }

  Ok(parsed_cluster)
}

/// Parse cluster configurations from a YAML file
pub fn parse_clusters_configs_from_file(root: &Path) -> Result<Vec<NewClusterConfig>, ParserError> {
  let variables = get_include_variables(root)?;
  let yaml = load_yaml_from_file(root)?;

  let clusters = lookup_mapping(&yaml, "clusters").map_err(|_| ParserError::EmptyClusterConfig)?;
  let mut parsed_clusters = vec![];
  for (cluster_name, configs) in clusters {
    parsed_clusters.push(parse_cluster(
      to_string(cluster_name)?,
      configs,
      &variables,
    )?);
  }
  Ok(parsed_clusters)
}
