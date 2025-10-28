use std::collections::HashMap;

use pyo3::Python;

use crate::core::{
  cluster_configs::ClusterConfig,
  database::models::{Cluster, Job},
  jobs::{
    tests::{create_test_cluster, create_test_config},
    variables::{get_variables_dependency, scalar_to_string},
  },
  parsers::variables::{BasicVar, ClusterMap, CompleteVar, Scalar, Variable},
};

// Helper function to create a variable
fn test_variable(name: &str, contents: CompleteVar) -> Variable {
  Variable {
    name: name.to_string(),
    contents,
  }
}

#[test]
fn test_get_variables_dependency_simple() {
  let s = String::from("Hello ${NAME}");
  let deps = get_variables_dependency(&s);
  assert_eq!(deps, Some(vec!["NAME"]));
}

#[test]
fn test_get_variables_dependency_multiple() {
  let s = String::from("${VAR1} and ${VAR2} and ${VAR3}");
  let deps = get_variables_dependency(&s);
  assert_eq!(deps, Some(vec!["VAR1", "VAR2", "VAR3"]));
}

#[test]
fn test_get_variables_dependency_none() {
  let s = String::from("No variables here");
  let deps = get_variables_dependency(&s);
  assert_eq!(deps, None);
}

#[test]
fn test_get_variables_dependency_empty_braces() {
  let s = String::from("Empty ${}");
  let deps = get_variables_dependency(&s);
  assert_eq!(deps, None);
}

#[test]
fn test_get_variables_dependency_nested() {
  let s = String::from("${MAP}[${KEY}]");
  let deps = get_variables_dependency(&s);
  assert_eq!(deps, Some(vec!["MAP", "KEY"]));
}

#[test]
fn test_scalar_to_string() {
  assert_eq!(
    scalar_to_string(&Scalar::String("test".to_string())),
    Some("test".to_string())
  );
  assert_eq!(scalar_to_string(&Scalar::Int(42)), Some("42".to_string()));
  assert_eq!(
    scalar_to_string(&Scalar::Float(3.14)),
    Some("3.14".to_string())
  );
  assert_eq!(
    scalar_to_string(&Scalar::Bool(true)),
    Some("true".to_string())
  );
  assert_eq!(
    scalar_to_string(&Scalar::File("file.txt".to_string())),
    Some("file.txt".to_string())
  );
}

#[test]
fn test_simple_variable_substitution() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable(
    "NAME",
    CompleteVar::Scalar(Scalar::String("World".to_string())),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "Hello ${NAME}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "Hello World");
}

#[test]
fn test_multiple_scalar_variables() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![
    test_variable(
      "GREETING",
      CompleteVar::Scalar(Scalar::String("Hello".to_string())),
    ),
    test_variable(
      "NAME",
      CompleteVar::Scalar(Scalar::String("World".to_string())),
    ),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "${GREETING} ${NAME}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "Hello World");
}

#[test]
fn test_list_variable_cartesian_product() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable(
    "NUM",
    CompleteVar::List(vec![Scalar::Int(1), Scalar::Int(2), Scalar::Int(3)]),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "Value: ${NUM}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 3);
  assert_eq!(jobs[0].command, "Value: 1");
  assert_eq!(jobs[1].command, "Value: 2");
  assert_eq!(jobs[2].command, "Value: 3");
}

#[test]
fn test_multiple_list_variables_cartesian_product() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![
    test_variable(
      "X",
      CompleteVar::List(vec![
        Scalar::String("a".to_string()),
        Scalar::String("b".to_string()),
      ]),
    ),
    test_variable("Y", CompleteVar::List(vec![Scalar::Int(1), Scalar::Int(2)])),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "${X}-${Y}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 4);
  let commands: Vec<_> = jobs.iter().map(|j| j.command.as_str()).collect();
  assert!(commands.contains(&"a-1"));
  assert!(commands.contains(&"a-2"));
  assert!(commands.contains(&"b-1"));
  assert!(commands.contains(&"b-2"));
}

#[test]
fn test_cluster_map_resolution() {
  let mut cl = create_test_cluster(1);
  cl.cluster_name = "cluster_a".to_string();
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);

  let mut per_cluster = HashMap::new();
  per_cluster.insert(
    "cluster_a".to_string(),
    BasicVar::Scalar(Scalar::String("value_a".to_string())),
  );
  per_cluster.insert(
    "cluster_b".to_string(),
    BasicVar::Scalar(Scalar::String("value_b".to_string())),
  );

  let variables = vec![test_variable(
    "CONFIG",
    CompleteVar::ClusterMap(ClusterMap {
      default: Some(BasicVar::Scalar(Scalar::String("default".to_string()))),
      per_cluster,
    }),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "Config: ${CONFIG}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "Config: value_a");
}

#[test]
fn test_cluster_map_default_value() {
  let mut cl = create_test_cluster(1);
  cl.cluster_name = "cluster_c".to_string();
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);

  let mut per_cluster = HashMap::new();
  per_cluster.insert(
    "cluster_a".to_string(),
    BasicVar::Scalar(Scalar::String("value_a".to_string())),
  );

  let variables = vec![test_variable(
    "CONFIG",
    CompleteVar::ClusterMap(ClusterMap {
      default: Some(BasicVar::Scalar(Scalar::String("default".to_string()))),
      per_cluster,
    }),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "Config: ${CONFIG}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "Config: default");
}

#[test]
fn test_cluster_map_with_list() {
  let mut cl = create_test_cluster(1);
  cl.cluster_name = "cluster_a".to_string();
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);

  let mut per_cluster = HashMap::new();
  per_cluster.insert(
    "cluster_a".to_string(),
    BasicVar::List(vec![Scalar::Int(1), Scalar::Int(2)]),
  );

  let variables = vec![test_variable(
    "VALUES",
    CompleteVar::ClusterMap(ClusterMap {
      default: None,
      per_cluster,
    }),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "Value: ${VALUES}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 2);
  assert_eq!(jobs[0].command, "Value: 1");
  assert_eq!(jobs[1].command, "Value: 2");
}

#[test]
fn test_standard_map_substitution() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);

  let mut map = HashMap::new();
  map.insert(
    "key1".to_string(),
    BasicVar::Scalar(Scalar::String("value1".to_string())),
  );
  map.insert(
    "key2".to_string(),
    BasicVar::Scalar(Scalar::String("value2".to_string())),
  );

  let variables = vec![
    test_variable("MAP", CompleteVar::StandardMap(map)),
    test_variable(
      "KEY",
      CompleteVar::Scalar(Scalar::String("key1".to_string())),
    ),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "${MAP}[${KEY}]".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "value1");
}

#[test]
fn test_standard_map_literal_key() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);

  let mut map = HashMap::new();
  map.insert(
    "literal".to_string(),
    BasicVar::Scalar(Scalar::String("result".to_string())),
  );

  let variables = vec![test_variable("MAP", CompleteVar::StandardMap(map))];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "${MAP}[literal]".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "result");
}

#[test]
fn test_preprocess_and_postprocess() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable(
    "NAME",
    CompleteVar::Scalar(Scalar::String("test".to_string())),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "main ${NAME}".to_string(),
    Some("pre ${NAME}".to_string()),
    Some("post ${NAME}".to_string()),
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "main test");
  assert_eq!(jobs[0].preprocess, Some("pre test".to_string()));
  assert_eq!(jobs[0].postprocess, Some("post test".to_string()));
}

#[test]
fn test_mixed_literal_and_variable() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable(
    "CMD",
    CompleteVar::Scalar(Scalar::String("run".to_string())),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "./exec_${CMD}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "./exec_run");
}

#[test]
fn test_python_evaluation_simple() {
  Python::initialize(); // FIXME check if this is not a workaround
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![
    test_variable("FROM", CompleteVar::Scalar(Scalar::Int(0))),
    test_variable("TO", CompleteVar::Scalar(Scalar::Int(3))),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "@py [str(v) for v in range(${FROM}, ${TO})]".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "['0', '1', '2']");
}

#[test]
fn test_python_with_header() {
  Python::initialize(); // FIXME check if this is not a workaround
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable("VAL", CompleteVar::Scalar(Scalar::Int(5)))];

  let header = "def double(x): return x * 2".to_string();

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "@py double(${VAL})".to_string(),
    None,
    None,
    Some(header),
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "10");
}

#[test]
fn test_dependency_graph_simple() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![
    test_variable(
      "BASE",
      CompleteVar::Scalar(Scalar::String("hello".to_string())),
    ),
    test_variable(
      "DERIVED",
      CompleteVar::Scalar(Scalar::String("${BASE}_world".to_string())),
    ),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "${DERIVED}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "hello_world");
}

#[test]
fn test_empty_variables() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "static command".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "static command");
}

#[test]
fn test_variable_storage_in_job() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![
    test_variable(
      "VAR1",
      CompleteVar::Scalar(Scalar::String("value1".to_string())),
    ),
    test_variable("VAR2", CompleteVar::Scalar(Scalar::Int(42))),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "${VAR1} ${VAR2}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(
    jobs[0].variables.get("VAR1"),
    Some(&serde_json::Value::String("value1".to_string()))
  );
  assert_eq!(
    jobs[0].variables.get("VAR2"),
    Some(&serde_json::Value::String("42".to_string()))
  );
}

#[test]
fn test_cartesian_product_with_three_lists() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![
    test_variable(
      "A",
      CompleteVar::List(vec![
        Scalar::String("a1".to_string()),
        Scalar::String("a2".to_string()),
      ]),
    ),
    test_variable(
      "B",
      CompleteVar::List(vec![
        Scalar::String("b1".to_string()),
        Scalar::String("b2".to_string()),
      ]),
    ),
    test_variable(
      "C",
      CompleteVar::List(vec![
        Scalar::String("c1".to_string()),
        Scalar::String("c2".to_string()),
      ]),
    ),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "${A}-${B}-${C}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 8); // 2 * 2 * 2 = 8
}

#[test]
fn test_bool_scalar_type() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable(
    "FLAG",
    CompleteVar::Scalar(Scalar::Bool(true)),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "flag=${FLAG}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "flag=true");
}

#[test]
fn test_float_scalar_type() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable(
    "PI",
    CompleteVar::Scalar(Scalar::Float(3.14159)),
  )];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "pi=${PI}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "pi=3.14159");
}

#[test]
fn test_file_and_directory_types() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![
    test_variable(
      "FILE",
      CompleteVar::Scalar(Scalar::File("input.txt".to_string())),
    ),
    test_variable(
      "DIR",
      CompleteVar::Scalar(Scalar::Directory("/data".to_string())),
    ),
  ];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "process ${FILE} in ${DIR}".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "process input.txt in /data");
}

#[test]
fn test_multiple_python_expressions() {
  let cl = create_test_cluster(1);
  let cf = create_test_config(1);
  let cluster = ClusterConfig::new(&cl, &cf);
  let variables = vec![test_variable("N", CompleteVar::Scalar(Scalar::Int(3)))];

  let jobs = Job::generate_from(
    &cluster,
    &variables,
    "@py ${N} * 2 @py ${N} + 1".to_string(),
    None,
    None,
    None,
  );

  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].command, "6 4");
}
