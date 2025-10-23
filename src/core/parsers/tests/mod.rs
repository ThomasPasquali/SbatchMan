use crate::core::parsers::{
  includes::get_include_variables,
  variables::{BasicVar, CompleteVar, Scalar},
};

use super::*;
use std::{
  collections::HashMap,
  path::{Path, PathBuf},
};

fn get_test_path(p: &str) -> PathBuf {
  PathBuf::from("src/core/parsers/tests/files").join(p)
}

#[test]
fn test_get_include_variables_simple() {
  let path = get_test_path("variables.yaml");

  let result = get_include_variables(&path);
  let variables = result.unwrap();

  // variables.yaml includes recursive_vars.yaml
  assert_eq!(variables.len(), 8);
  assert!(
    matches!(variables["dataset"].contents, CompleteVar::Scalar(Scalar::Directory(ref dir)) if dir == "datasets/")
  );
  assert!(
    matches!(variables["mode"].contents, CompleteVar::Scalar(Scalar::File(ref s)) if s == "modes.txt")
  );
  assert!(
    matches!(variables["implementation"].contents, CompleteVar::List(ref l) if l == &vec![
      Scalar::Bool(true),
      Scalar::Int(-5),
      Scalar::Float(-5.0),
    ])
  );
  assert!(
    matches!(variables["args"].contents, CompleteVar::StandardMap(ref m) if m == &HashMap::from([
      ("impl1".to_string(), BasicVar::Scalar(Scalar::String("--arg-for-impl1 --another-for-impl1".to_string()))),
      ("impl2".to_string(), BasicVar::Scalar(Scalar::String("--arg-for-impl2".to_string())))
    ]))
  );
  assert!(
    matches!(variables["nodes"].contents, CompleteVar::ClusterMap(ref cm) if cm.default == Some(BasicVar::List(
      vec![
        Scalar::Int(1),
        Scalar::Int(2),
        Scalar::Int(4),
        Scalar::Int(8),
      ]
    )) &&
      cm.per_cluster == HashMap::from([
        ("clusterA".to_string(), BasicVar::List(vec![Scalar::Int(1)])),
        ("clusterB".to_string(), BasicVar::List(vec![Scalar::Int(1), Scalar::Int(2)]))
      ])
    )
  );
  assert!(
    matches!(variables["to_override"].contents, CompleteVar::Scalar(Scalar::String(ref s)) if s == "NOT OVERWRITTEN")
  );
  assert!(
    matches!(variables["recursive"].contents, CompleteVar::Scalar(Scalar::String(ref s)) if s == "ok")
  );
}

#[test]
fn test_get_include_variables_override() {
  let path = get_test_path("jobs.yaml");

  let result = get_include_variables(&path);
  assert!(result.is_ok());
  let variables = result.unwrap();

  assert_eq!(variables.len(), 8);
  assert!(variables.contains_key("to_override"));

  let to_override_var = variables.get("to_override").unwrap();
  if let CompleteVar::Scalar(Scalar::String(s)) = &to_override_var.contents {
    assert_eq!(s, "OVERWRITTEN");
  } else {
    panic!("'to_override' variable has wrong type");
  }

  // Check a variable from the included file to ensure it's there
  assert!(variables.contains_key("dataset"));
}

#[test]
fn test_get_include_variables_multiple_includes() {
  let path = get_test_path("clusters_configs.yaml");

  let result = get_include_variables(&path);
  assert!(result.is_ok());
  let variables = result.unwrap();

  // clusters_configs.yaml includes variables.yaml (and its include) and subdir/more_variables.yaml
  // It also defines its own variables.
  // variables.yaml -> 7 vars + 1 from recursive_vars.yaml
  // subdir/more_variables.yaml -> 1 var
  // clusters_configs.yaml -> 3 vars
  // Total = 8 + 1 + 3 = 12
  assert_eq!(variables.len(), 12);

  // From clusters_configs.yaml
  assert!(variables.contains_key("partition"));
  assert!(variables.contains_key("qos"));
  assert!(variables.contains_key("task_cpus"));

  // From variables.yaml
  assert!(variables.contains_key("dataset"));
  assert!(variables.contains_key("nodes"));

  // From recursive_vars.yaml (via variables.yaml)
  assert!(variables.contains_key("recursive"));

  // From subdir/more_variables.yaml
  assert!(variables.contains_key("another_var"));
  let another_var = variables.get("another_var").unwrap();
  if let CompleteVar::Scalar(Scalar::String(s)) = &another_var.contents {
    assert_eq!(s, "value");
  } else {
    panic!("'another_var' variable has wrong type");
  }

  let to_override_var = variables.get("to_override1").unwrap();
  if let CompleteVar::Scalar(Scalar::String(s)) = &to_override_var.contents {
    assert_eq!(s, "OVERWRITTEN");
  } else {
    panic!("'to_override' variable has wrong type");
  }
}

#[test]
fn test_get_include_variables_no_includes() {
  let path = get_test_path("recursive_vars.yaml");

  let result = get_include_variables(&path);
  assert!(result.is_ok());
  let variables = result.unwrap();

  assert_eq!(variables.len(), 1);
  assert!(variables.contains_key("recursive"));
}

#[test]
fn test_get_include_variables_file_not_found() {
  let path = Path::new("/tmp/dummy.yaml");

  let result = get_include_variables(&path);
  assert!(result.is_err());
  match result.err().unwrap() {
    ParserError::IoError(_) => {} // Correct error type
    e => panic!("Expected IoError, got {:?}", e),
  }
}

#[test]
fn test_get_include_variables_include_empty() {
  let path = get_test_path("include_empty.yaml");

  let result = get_include_variables(&path);
  assert!(matches!(
    result.err(),
    Some(ParserError::IncludeWrongType(..))
  ));
}

#[test]
fn test_get_include_variables_include_number() {
  let path = get_test_path("include_number.yaml");

  let result = get_include_variables(&path);
  assert!(matches!(
    result.err(),
    Some(ParserError::IncludeWrongType(..))
  ));
}

#[test]
fn test_get_include_variables_include_missing_file() {
  let path = get_test_path("include_missing_file.yaml");
  let result = get_include_variables(&path);
  assert!(result.is_err());
  match result.err().unwrap() {
    ParserError::IoError(_) => {} // Correct error type
    e => panic!("Expected IoError, got {:?}", e),
  }
}

#[test]
fn test_get_include_variables_no_include() {
  let path = get_test_path("no_include.yaml");

  let result = get_include_variables(&path);
  assert!(result.is_ok());
  let variables = result.unwrap();

  assert_eq!(variables.len(), 2);
  assert!(variables.contains_key("VAR1"));
  assert!(variables.contains_key("VAR2"));
}

fn test_get_include_variables_circular_include(path: &Path) {
  let result = get_include_variables(&path);
  assert!(result.is_err());
  match result.err().unwrap() {
    ParserError::CircularInclude(_) => {} // Correct error type
    e => panic!("Expected CircularInclude, got {:?}", e),
  }
}

#[test]
fn test_get_include_variables_circular_1() {
  let path = get_test_path("circular1.yaml");
  test_get_include_variables_circular_include(&path);
}

#[test]
fn test_get_include_variables_circular_2() {
  let path = get_test_path("circular2.yaml");
  test_get_include_variables_circular_include(&path);
}

#[test]
fn test_get_include_variables_circular_3() {
  let path = get_test_path("circular3.yaml");
  test_get_include_variables_circular_include(&path);
}

#[test]
fn test_get_include_variables_circular_4() {
  let path = get_test_path("circular4.yaml");
  test_get_include_variables_circular_include(&path);
}
