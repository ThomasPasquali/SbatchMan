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

macro_rules! assert_is_dir {
  ($var:expr, $expected:expr) => {
    match &$var {
      CompleteVar::BasicVar(BasicVar::Scalar(Scalar::Directory(s))) => {
        assert_eq!(s, $expected, "Directory path mismatch");
      }
      val => {
        panic!("Expected Scalar::Directory, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_is_file {
  ($var:expr, $expected:expr) => {
    match &$var {
      CompleteVar::BasicVar(BasicVar::Scalar(Scalar::File(s))) => {
        assert_eq!(s, $expected, "File path mismatch");
      }
      val => {
        panic!("Expected Scalar::File, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_is_python {
  ($var:expr, $expected:expr) => {
    match &$var {
      CompleteVar::BasicVar(BasicVar::Python(s)) => {
        assert_eq!(s, $expected, "Python code mismatch");
      }
      val => {
        panic!("Expected Scalar::Python, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_is_string {
  ($var:expr, $expected:expr) => {
    match &$var {
      CompleteVar::BasicVar(BasicVar::Scalar(Scalar::String(s))) => {
        assert_eq!(s, $expected, "String value mismatch");
      }
      val => {
        panic!("Expected Scalar::String, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_is_list {
  ($var:expr, $expected:expr) => {
    match $var {
      CompleteVar::BasicVar(BasicVar::List(l)) => {
        assert_eq!(l, &$expected, "List value mismatch");
      }
      val => {
        panic!("Expected BasicVar::List, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_is_standard_map {
  ($var:expr, $expected:expr) => {
    match $var {
      CompleteVar::StandardMap(m) => {
        assert_eq!(m, &$expected, "StandardMap value mismatch");
      }
      val => {
        panic!("Expected CompleteVar::StandardMap, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_is_cluster_map {
  ($var:expr, $expected_default:expr, $expected_per_cluster:expr) => {
    match $var {
      CompleteVar::ClusterMap(cm) => {
        assert_eq!(
          cm.default, $expected_default,
          "ClusterMap default value mismatch"
        );
        assert_eq!(
          cm.per_cluster, $expected_per_cluster,
          "ClusterMap per_cluster value mismatch"
        );
      }
      val => {
        panic!("Expected CompleteVar::ClusterMap, found {:?}", val);
      }
    }
  };
}

#[test]
fn test_get_include_variables_simple() {
  let path = get_test_path("variables.yaml");

  let result = get_include_variables(&path);
  let variables = result.unwrap();

  // variables.yaml includes recursive_vars.yaml
  assert_eq!(variables.len(), 8);
  assert_is_dir!(variables["dataset"].contents, "datasets/");
  assert_is_file!(variables["mode"].contents, "modes.txt");
  assert_is_list!(
    &variables["implementation"].contents,
    vec![Scalar::Bool(true), Scalar::Int(-5), Scalar::Float(-5.0),]
  );
  assert_is_standard_map!(
    &variables["args"].contents,
    HashMap::from([
      (
        "impl1".to_string(),
        BasicVar::Scalar(Scalar::String(
          "--arg-for-impl1 --another-for-impl1".to_string()
        ))
      ),
      (
        "impl2".to_string(),
        BasicVar::Scalar(Scalar::String("--arg-for-impl2".to_string()))
      ),
    ])
  );
  assert_is_cluster_map!(
    &variables["nodes"].contents,
    Some(BasicVar::List(vec![
      Scalar::Int(1),
      Scalar::Int(2),
      Scalar::Int(4),
      Scalar::Int(8),
    ])),
    HashMap::from([
      (
        "clusterA".to_string(),
        BasicVar::List(vec![Scalar::Int(1)])
      ),
      (
        "clusterB".to_string(),
        BasicVar::List(vec![Scalar::Int(1), Scalar::Int(2)])
      ),
    ])
  );
  assert_is_string!(variables["to_override"].contents, "NOT OVERWRITTEN");
  assert_is_string!(variables["recursive"].contents, "ok");
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
  assert_is_string!(to_override_var.contents, "OVERWRITTEN");

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
  assert_is_string!(another_var.contents, "value");

  let to_override_var = variables.get("to_override1").unwrap();
  assert_is_string!(to_override_var.contents, "OVERWRITTEN");
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
fn test_special_types() {
  let path = get_test_path("special_types.yaml");

  let result = get_include_variables(&path);
  assert!(result.is_ok());
  let variables = result.unwrap();

  assert_eq!(variables.len(), 5);

  // Test !dir
  assert!(
    matches!(variables["dataset_dir"].contents, CompleteVar::BasicVar(BasicVar::Scalar(Scalar::Directory(ref s))) if s == "datasets/images")
  );

  // Test !file
  assert_is_file!(variables["gpu_list"].contents, "gpus.txt");

  // Test !python with multiline string
  let expected_python_code1 = "# This Python code generates a list of values\nbase = 10\nreturn [base * i for i in range(1, 6)]\n";
  assert_is_python!(variables["generated_values"].contents, expected_python_code1);

  // Test !python with single line string
  let expected_python_code2 =
    "# This Python code returns a single value\nreturn \"single_generated_value\"\n";
  assert_is_python!(variables["single_value"].contents, expected_python_code2);

  // Test !python with variable reference
  let expected_python_code3 = "# This Python code references existing variables\ndataset_count = len($dataset_dir)\nif dataset_count == 0:\n  return \"No datasets found\"\nreturn f\"Number of datasets: {dataset_count}\"\n";
  assert_is_python!(variables["reference_existing"].contents, expected_python_code3);
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
    ParserError::MultipleInclude(_) => {} // Correct error type
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
