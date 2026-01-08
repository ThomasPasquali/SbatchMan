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
    multi_hashmap::MultiHashMap,
    template_parser::Template,
    variable_parser::{CompleteVar, parse_variables as parse_variables_hashmap},
    yaml_parser::{
      check_mapping_keys, load_yaml_from_file, lookup_mapping, lookup_sequence, lookup_str, to_mapping, to_string, value_from_str, yaml_lookup
    },
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

// Takes as input a mapping and returns an object containing the list of config entries and env variables
fn parse_params(
  yaml: &YamlOwned,
  scheduler: Scheduler,
  all_config_entries: &mut MultiHashMap<String, Template>,
  all_env_variables: &mut MultiHashMap<String, Template>,
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
        env_variables.insert(key, Template::from_str(&value)?);
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
      config_entries.insert(key, Template::from_str(&value)?);
    }
  }
  all_env_variables.push(env_variables);
  all_config_entries.push(config_entries);
  Ok(())
}

fn parse_config(
  yaml: &YamlOwned,
  scheduler: Scheduler,
  cluster_name: String,
  variables: &mut MultiHashMap<String, CompleteVar>,
  config_entries: &mut MultiHashMap<String, Template>,
  env_variables: &mut MultiHashMap<String, Template>,
) -> Result<Vec<NewConfig>, ParserError> {
  let required_keys = vec!["name"];
  let optional_keys = vec!["variables", "params"];
  check_mapping_keys(&yaml, &required_keys, &optional_keys)?;

  // Parse variables
  parse_variables(yaml, variables)?;

  // Parse params (config entries and env)
  parse_params(yaml, scheduler, config_entries, env_variables)?;

  let name_str = lookup_str(yaml, "name")?;
  let name_template = Template::from_str(&name_str)?;

  let mut generator = CombinationGenerator::new(variables, &cluster_name);
  generator.register_template(&name_template)?;
  // Register all templates
  for (_, template) in config_entries.iter() {
    generator.register_template(template)?;
  }
  for (_, template) in env_variables.iter() {
    generator.register_template(template)?;
  }

  let mut combinations = generator.try_iter()?;
  let mut flags: HashMap<String, String> = HashMap::new();
  let mut env: HashMap<String, String> = HashMap::new();
  let mut configs = vec![];

  while combinations.next().is_some() {
    // Inefficient as we re-evaluate all templates for each combination.
    // Since the generator already knows which variables changed, we could optimize this by re-evaluating only the affected templates.
    for (name, template) in config_entries.iter() {
      flags.insert(name.clone(), template.render(&combinations)?);
    }
    for (name, template) in env_variables.iter() {
      env.insert(name.clone(), template.render(&combinations)?);
    }
    configs.push(NewConfig {
      config_name: name_template.render(&combinations)?,
      cluster_id: 0, // to be filled when inserting in the DB
      flags: json!(flags),
      env: json!(env),
    });
  }

  // Pop config-level entries
  variables.pop();
  config_entries.pop();
  env_variables.pop();

  Ok(configs)
}

/// Parses a single cluster configuration from YAML node.
fn parse_cluster(
  cluster_name: String,
  yaml: &YamlOwned,
  variables: &mut MultiHashMap<String, CompleteVar>,
) -> Result<NewClusterConfig, ParserError> {
  let required_keys = vec!["scheduler", "configs"];
  let optional_keys = vec!["max_jobs", "params", "variables"];
  check_mapping_keys(&yaml, &required_keys, &optional_keys)?;

  // Parse scheduler
  let scheduler_str = lookup_str(yaml, "scheduler")?;
  let scheduler = Scheduler::from_str(&scheduler_str)
    .map_err(|_| ParserError::InvalidScheduler(scheduler_str.clone()))?;
  let mut config_entries = MultiHashMap::new();
  let mut env_variables = MultiHashMap::new();
  // Parse cluster-level variables
  parse_variables(yaml, variables)?;

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

/** Reads YAML file that defines cluster configurations. Returns a vector of parsed clusters with their configurations.
 *
 * High level description of the parsing logic:
 * - parse top-level variables and add them to the variable multi-hashmap
 * - for each cluster in the clusters configuration:
 *   - parse cluster-level variables/config entries/env and add them to the respective multi-hashmaps
 *   - for each config in the cluster:
 *     - parse config-level variables/config entries/env and add them to the respective multi-hashmaps
 *     - iterate over config and env multi-hashmaps to build the dependency graph of variables
 *     - sort the dependency graph topologically
 *     - run combination generation procedure
 */
pub fn parse_clusters_configs_from_file(root: &Path) -> Result<Vec<NewClusterConfig>, ParserError> {
  let yaml = load_yaml_from_file(root)?;
  let required_keys = vec!["clusters"];
  let optional_keys = vec!["include", "variables"];
  check_mapping_keys(&yaml, &required_keys, &optional_keys)?;

  let mut variables = MultiHashMap::new();
  parse_include_variables(&yaml, root, &mut variables)?;

  let mut parsed_clusters = vec![];
  for (cluster_name, configs) in lookup_mapping(&yaml, "clusters")? {
    parsed_clusters.push(parse_cluster(
      to_string(cluster_name)?,
      configs,
      &mut variables,
    )?);
  }
  Ok(parsed_clusters)
}

fn parse_variables(
  config: &YamlOwned,
  variables: &mut MultiHashMap<String, CompleteVar>,
) -> Result<(), ParserError> {
  let mut temp_variables = HashMap::new();
  parse_variables_hashmap(config, &mut temp_variables)?;
  variables.push(temp_variables);
  Ok(())
}