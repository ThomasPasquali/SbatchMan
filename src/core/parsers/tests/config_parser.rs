use crate::core::parsers::ParserError;
use crate::core::parsers::config_parser::parse_clusters_configs_from_file;
use crate::core::database::models::Scheduler;
use crate::core::parsers::tests::get_test_path;

#[test]
fn test_success() {
  let file = get_test_path("clusters_configs.yaml");

  let clusters = parse_clusters_configs_from_file(&file).expect("Failed to parse valid config");

  assert_eq!(clusters.len(), 3);

  // Cluster A
  let cluster_a = clusters.iter().find(|c| c.cluster.cluster_name == "clusterA").unwrap();
  assert_eq!(cluster_a.cluster.scheduler, Scheduler::Slurm);
  assert_eq!(cluster_a.configs.len(), 12);

  // Check unique config names
  let names: Vec<&str> = cluster_a.configs.iter().map(|c| c.config_name.as_str()).collect();
  assert!(names.contains(&"config_1-N_1-CPU_partition_A1"));
  assert!(names.contains(&"config_1-N_8-CPU_partition_A2"));
  assert!(names.contains(&"budget_1-N_4-CPU_partition_A1"));

  // Cluster B
  let cluster_b = clusters.iter().find(|c| c.cluster.cluster_name == "clusterB").unwrap();
  assert_eq!(cluster_b.cluster.scheduler, Scheduler::Pbs);
  // nodes: [1, 2], task_cpus: [1, 4, 8]. 2*3 = 6.
  assert_eq!(cluster_b.configs.len(), 6);
  
  let names_b: Vec<&str> = cluster_b.configs.iter().map(|c| c.config_name.as_str()).collect();
  assert!(names_b.contains(&"weird_but_plausible_1-N_1-CPU"));
  assert!(names_b.contains(&"weird_but_plausible_2-N_8-CPU"));

  // Cluster C
  let cluster_c = clusters.iter().find(|c| c.cluster.cluster_name == "clusterC").unwrap();
  assert_eq!(cluster_c.cluster.scheduler, Scheduler::Local);
  // nodes: [1], task_cpus: [1, 4, 8]. 1*3 = 3.
  assert_eq!(cluster_c.configs.len(), 3);
}

#[test]
fn test_invalid_parameter() {
  let file = get_test_path("clusters_invalid_parameter.yaml");

  let result = parse_clusters_configs_from_file(&file);
  assert!(matches!(result, Err(ParserError::InvalidParameterForScheduler(_, _))));
}
