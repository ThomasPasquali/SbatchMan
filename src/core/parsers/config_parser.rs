/// config_parser.rs
/// Provides the functionality to parse a cluster configuration file.

use std::{
  collections::{HashMap, HashSet},
  path::Path,
  str::FromStr,
};

use once_cell::sync::Lazy;
use saphyr::YamlOwned;
use serde_json::json;

use crate::core::{
  database::models::{NewCluster, NewClusterConfig, NewConfig, Scheduler},
  parsers::{
    ParserError,
    combination_generator::CombinationGenerator,
    includes::parse_include_variables,
    multi_hashmap::LayeredHashMap,
    entry_parser::ParsedEntry,
    variable_parser::{CompleteVar, parse_variables},
    yaml_parser::{
      check_invalid_keys, load_yaml_from_file, lookup_mapping, lookup_sequence, lookup_str,
      to_mapping, to_string, value_from_str, yaml_lookup,
    },
  },
};

/// Specifies the configuration parameters supported by the scheduler
/// TODO: extend the definitions to include also the description of each parameter, so that it can be used also to generate the help messages
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

// Parses the configuration parameters and environmental variables for a given scheduler into a ParsedEntry object and adds them to the layered hashmap.
fn parse_params(
  yaml: &YamlOwned,
  scheduler: Scheduler,
  all_config_entries: &mut LayeredHashMap<String, ParsedEntry>,
  all_env_variables: &mut LayeredHashMap<String, ParsedEntry>,
) -> Result<(), ParserError> {
  let mut config_entries = HashMap::new();
  let mut env_variables = HashMap::new();
  if let Ok(yaml) = lookup_mapping(yaml, "params") {
    if let Some(env_node) = yaml.get(&value_from_str("env")) {
      // Parse env variables
      let env_mapping = to_mapping(env_node)?;
      for (key_node, value_node) in env_mapping {
        let key = to_string(key_node)?;
        let value = to_string(value_node)?;
        env_variables.insert(key, ParsedEntry::from_str(&value)?);
      }
    }

    for (key_node, value_node) in yaml {
      let key = to_string(key_node)?;
      // Skip env as it has been already processed
      if key == "env" {
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
      config_entries.insert(key, ParsedEntry::from_str(&value)?);
    }
  }
  all_env_variables.push(env_variables);
  all_config_entries.push(config_entries);
  Ok(())
}

/// Parse a complete cluster configuration file.
fn parse_config(
  yaml: &YamlOwned,
  path: &Path,
  scheduler: Scheduler,
  cluster_name: String,
  variables: &mut LayeredHashMap<String, CompleteVar>,
  config_entries: &mut LayeredHashMap<String, ParsedEntry>,
  env_variables: &mut LayeredHashMap<String, ParsedEntry>,
) -> Result<Vec<NewConfig>, ParserError> {
  // Check for invalid keys. An error is returned immediately if a key is not recognized.
  let required_keys = vec!["name"];
  let optional_keys = vec!["variables", "params"];
  check_invalid_keys(&yaml, &required_keys, &optional_keys)?;

  // Parse variables
  parse_variables(yaml, variables, path)?;

  // Parse params (config entries and env)
  parse_params(yaml, scheduler, config_entries, env_variables)?;

  let name_str = lookup_str(yaml, "name")?;
  let config_name = ParsedEntry::from_str(&name_str)?;

  // The combination generator is used to generate all possible combinations, given the variables found in the configuration name, configuration parameters and environmental variables.
  let mut generator = CombinationGenerator::new(variables, &cluster_name);
  // Finds the variables used in the strings and registers them with the combination generator
  generator.register_parsed_entry(&config_name)?;
  for (_, entry) in config_entries.iter() {
    generator.register_parsed_entry(entry)?;
  }
  for (_, entry) in env_variables.iter() {
    generator.register_parsed_entry(entry)?;
  }

  let mut combinations = generator.try_iter()?;
  let mut configs = vec![]; // Stores the generated configurations
  let mut config_names: HashSet<String> = HashSet::new(); // Associates the configuration names with their respective configurations. Used to check for duplicates.

  while combinations.next().is_some() {
    // Generate a new configuration for each combination returned by the combination generator.
    // At each iteration, all configuration entries and environmental variables are re-evaluated.
    // This may be optimized in the future: since the iterator knows which variables are changes, we might re-evaluate just the entries that have changed. We need to find an efficient way of communicating this information from the combination generator, maybe by using the return value from the .next() call of the Rust iterator. Keep in mind that the variable values are anyways cached already, therefore the expected speedup is not very significant, as the more complicated variables such as Python nodes don't have to be computed every time.
    let mut flags: HashMap<String, String> = HashMap::new();
    let mut env: HashMap<String, String> = HashMap::new();
    for (name, entry) in config_entries.iter() {
      flags.insert(name.clone(), entry.render(&combinations)?);
    }
    for (name, entry) in env_variables.iter() {
      env.insert(name.clone(), entry.render(&combinations)?);
    }

    // Check for duplicate configs
    let config_name = config_name.render(&combinations)?;
    if let Some(_) = config_names.get(&config_name) {
      return Err(ParserError::DuplicateConfigName(config_name));
    }
    config_names.insert(config_name.clone());

    configs.push(NewConfig {
      config_name,
      cluster_id: 0, // to be filled when inserting in the DB
      flags: json!(flags),
      env: json!(env),
    });
  }

  // Pop the entries added in the layered hashmaps by this configuration
  variables.pop();
  config_entries.pop();
  env_variables.pop();

  Ok(configs)
}

/// Parses the entire cluster configuration from YAML node. Calls parse_config for parsing single cluster configurations.
fn parse_cluster(
  cluster_name: String,
  yaml: &YamlOwned,
  path: &Path,
  variables: &mut LayeredHashMap<String, CompleteVar>,
) -> Result<NewClusterConfig, ParserError> {
  let required_keys = vec!["scheduler", "configs"];
  let optional_keys = vec!["max_jobs", "params", "variables"];
  check_invalid_keys(&yaml, &required_keys, &optional_keys)?;

  // Parse scheduler
  let scheduler_str = lookup_str(yaml, "scheduler")?;
  let scheduler = Scheduler::from_str(&scheduler_str)
    .map_err(|_| ParserError::InvalidScheduler(scheduler_str.clone()))?;

  let mut config_entries = LayeredHashMap::new();
  let mut env_variables = LayeredHashMap::new();
  // Parse cluster-level variables
  parse_variables(yaml, variables, path)?;

  // Parse cluster-level default params (options and env)
  parse_params(yaml, scheduler, &mut config_entries, &mut env_variables)?;

  // Max jobs
  let max_jobs = yaml_lookup(yaml, "max_jobs")
    .and_then(|n| n.as_integer())
    .map(|i| i as i32);

  // Configs
  let mut parsed_cluster = NewClusterConfig {
    cluster: NewCluster {
      cluster_name: cluster_name.clone(),
      scheduler: scheduler,
      max_jobs,
    },
    configs: vec![],
  };

  let configs = lookup_sequence(yaml, "configs")?;
  for config in configs.iter() {
    parsed_cluster.configs.extend(parse_config(
      config,
      path,
      scheduler,
      cluster_name.clone(),
      variables,
      &mut config_entries,
      &mut env_variables,
    )?);
  }

  // Pop cluster-level entries
  variables.pop();
  config_entries.pop();
  env_variables.pop();

  Ok(parsed_cluster)
}

/// Reads YAML file that defines cluster configurations. Returns a vector of parsed clusters with their configurations.
/// High-level description of the parsing logic:
/// - parse top-level variables and add them to the variable layered hashmap
/// - for each cluster in the clusters configuration:
///   - parse cluster-level variables/config/env entries and add them to the respective layered hashmaps
///   - for each config in the cluster:
///     1. parse config-level variables/config/env entries and add them to the respective layered hashmaps
///     2. run combination generation procedure
pub fn parse_clusters_configs_from_file(root: &Path) -> Result<Vec<NewClusterConfig>, ParserError> {
  let yaml = load_yaml_from_file(root)?;
  let required_keys = vec!["clusters"];
  let optional_keys = vec!["include", "variables"];
  check_invalid_keys(&yaml, &required_keys, &optional_keys)?;

  let mut variables = LayeredHashMap::new();
  parse_include_variables(&yaml, root, &mut variables)?;

  let mut parsed_clusters = vec![];
  for (cluster_name, configs) in lookup_mapping(&yaml, "clusters")? {
    parsed_clusters.push(parse_cluster(
      to_string(cluster_name)?,
      configs,
      root,
      &mut variables,
    )?);
  }
  Ok(parsed_clusters)
}
