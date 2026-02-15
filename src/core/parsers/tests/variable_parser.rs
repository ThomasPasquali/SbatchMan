use crate::core::parsers::{
  ParserError, includes::parse_include_variables, multi_hashmap::LayeredHashMap, tests::get_test_path, variable_parser::{BasicVar, CompleteVar, MapKind, Scalar}, yaml_parser::load_yaml_from_file
};

macro_rules! assert_scalar_string {
  ($var:expr, $expected:expr) => {
    match $var {
      CompleteVar::BasicVar(BasicVar::Scalar(Scalar::String(s))) => {
        assert_eq!(s, $expected, "String value mismatch");
      }
      val => {
        panic!("Expected Scalar::String, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_is_python {
  ($var:expr) => {
    match $var {
      CompleteVar::Python(_) => {
        // Just checking variant existence for now as internal structure of PythonVar/Template is complex to assert equality on
      }
      val => {
        panic!("Expected CompleteVar::Python, found {:?}", val);
      }
    }
  };
}

macro_rules! assert_list_len {
  ($var:expr, $len:expr) => {
    match $var {
      CompleteVar::BasicVar(BasicVar::List(l)) => {
        assert_eq!(l.len(), $len);
      }
      val => {
        panic!("Expected BasicVar::List, found {:?}", val);
      }
    }
  };
}

fn load_yaml_and_include_variables(
  file_name: &str,
) -> Result<LayeredHashMap<String, CompleteVar>, ParserError> {
  let path = get_test_path(file_name);
  let yaml = load_yaml_from_file(&path)?;
  let mut variables = LayeredHashMap::new();
  parse_include_variables(&yaml, &path, &mut variables)?;
  Ok(variables)
}

#[test]
fn test_get_include_variables_simple() {
  let variables = load_yaml_and_include_variables("variables.yaml").unwrap();

  // variables.yaml includes recursive_vars.yaml
  // We can't easily check total count with MultiHashMap as it stores maps in a list.
  // But we can check specific keys.

  // Checking 'dataset' which uses !dir
  // In existing files/variables.yaml: dataset: !dir datasets/
  // In parse_tagged_basic_var("dir"), it uses read_dir.
  // We assume there are some files there.
  let dataset = variables.get("dataset").unwrap();
  assert_list_len!(dataset, 3);

  // Checking 'mode' which uses !file
  // mode: !file modes.txt
  // modes.txt content: "train\ntest\n"
  let mode = variables.get("mode").unwrap();
  match mode {
    CompleteVar::BasicVar(BasicVar::List(list)) => {
      assert_eq!(list.len(), 2);
      assert_eq!(list.get(0).unwrap().to_string(), "train");
      assert_eq!(list.get(1).unwrap().to_string(), "test");
    }
    _ => panic!("Expected List for mode"),
  }

  // implementation: [true, -5, -5.0]
  let implementation = variables.get("implementation").unwrap();
  match implementation {
    CompleteVar::BasicVar(BasicVar::List(list)) => {
      assert_eq!(list.len(), 3);
      assert_eq!(list.get(0).unwrap().to_string(), "true");
      assert_eq!(list.get(1).unwrap().to_string(), "-5");
      assert_eq!(list.get(2).unwrap().to_string(), "-5"); // f64 to_string might vary slightly but usually matches
    }
    _ => panic!("Expected List for implementation"),
  }

  // args: map ...
  let args = variables.get("args").unwrap();
  match args {
    CompleteVar::Map(map_var) => {
      assert_eq!(map_var.kind(), &MapKind::Standard);
      let val = map_var.get("impl1").unwrap();
      match val {
        BasicVar::Scalar(Scalar::String(s)) => assert_eq!(s, "--arg-for-impl1 --another-for-impl1"),
        _ => panic!("Expected string for impl1"),
      }
    }
    _ => panic!("Expected Map for args"),
  }

  // nodes: map ... per_cluster ...
  let nodes = variables.get("nodes").unwrap();
  match nodes {
    CompleteVar::Map(map_var) => {
      assert_eq!(map_var.kind(), &MapKind::Cluster);
      // Check clusterA
      let cluster_a = map_var.get("clusterA").unwrap();
      match cluster_a {
        BasicVar::List(l) => {
          assert_eq!(l.len(), 1);
          assert_eq!(l.get(0).unwrap().to_string(), "1");
        }
        _ => panic!("Expected List for clusterA"),
      }
    }
    _ => panic!("Expected Map for nodes"),
  }

  assert_scalar_string!(variables.get("to_override").unwrap(), "NOT OVERWRITTEN");
  assert_scalar_string!(variables.get("recursive").unwrap(), "ok");
}

#[test]
fn test_get_include_variables_override() {
  let variables = load_yaml_and_include_variables("jobs.yaml").unwrap();

  assert_scalar_string!(variables.get("to_override").unwrap(), "OVERWRITTEN");

  // Check a variable from the included file (variables.yaml) to ensure it's there
  assert!(variables.get("dataset").is_some());
}

#[test]
fn test_get_include_variables_multiple_includes() {
  let variables = load_yaml_and_include_variables("clusters_configs.yaml").unwrap();

  // From clusters_configs.yaml
  assert!(variables.get("partition").is_some());
  assert!(variables.get("qos").is_some());
  assert!(variables.get("task_cpus").is_some());

  // From variables.yaml
  assert!(variables.get("dataset").is_some());
  assert!(variables.get("nodes").is_some());

  // From recursive_vars.yaml (via variables.yaml)
  assert!(variables.get("recursive").is_some());

  // From subdir/more_variables.yaml
  let another_var = variables.get("another_var").unwrap();
  assert_scalar_string!(another_var, "value");

  let to_override_var = variables.get("to_override1").unwrap();
  assert_scalar_string!(to_override_var, "OVERWRITTEN");
}

#[test]
fn test_get_include_variables_no_includes() {
  let variables = load_yaml_and_include_variables("recursive_vars.yaml").unwrap();
  assert!(variables.get("recursive").is_some());
}

#[test]
fn test_get_include_variables_file_not_found() {
  let result = load_yaml_and_include_variables("tmp/dummy.yaml");
  assert!(matches!(result, Err(ParserError::IoError(_))));
}

#[test]
fn test_get_include_variables_include_empty() {
  let result = load_yaml_and_include_variables("include_empty.yaml");
  // Expect error
  assert!(matches!(result, Err(ParserError::IncludeWrongType(_))));
}

#[test]
fn test_get_include_variables_include_number() {
  let result = load_yaml_and_include_variables("include_number.yaml");
  // Assert that error is exactly IncludeWrongType
  assert!(matches!(result, Err(ParserError::IncludeWrongType(_))));
}

#[test]
fn test_special_types() {
  let variables = load_yaml_and_include_variables("special_types.yaml").unwrap();

  // Test !file
  // gpu_list: !file gpus.txt
  let gpu_list = variables.get("gpu_list").unwrap();
  assert_list_len!(gpu_list, 4);

  // Test !python
  assert_is_python!(variables.get("generated_values").unwrap());
  assert_is_python!(variables.get("single_value").unwrap());
  assert_is_python!(variables.get("reference_existing").unwrap());
}

#[test]
fn test_get_include_variables_include_missing_file() {
  let result = load_yaml_and_include_variables("include_missing_file.yaml");
  assert!(matches!(result, Err(ParserError::IoError(_))));
}

#[test]
fn test_file_not_found() {
  let result = load_yaml_and_include_variables("variable_file_not_found.yaml");
  assert!(matches!(result, Err(ParserError::FileReadError(_, _))));
}

#[test]
fn test_dir_not_found() {
  let result = load_yaml_and_include_variables("variable_dir_not_found.yaml");
  assert!(matches!(result, Err(ParserError::FileReadError(_, _))));
}

#[test]
fn test_circular_1() {
  let result = load_yaml_and_include_variables("circular_1.yaml");
  assert!(matches!(result, Err(ParserError::MultipleInclude(_))));
}

#[test]
fn test_circular_2() {
  let result = load_yaml_and_include_variables("circular_2.yaml");
  assert!(matches!(result, Err(ParserError::MultipleInclude(_))));
}

#[test]
fn test_circular_3() {
  let result = load_yaml_and_include_variables("circular_3.yaml");
  assert!(matches!(result, Err(ParserError::MultipleInclude(_))));
}

#[test]
fn test_circular_4() {
  let result = load_yaml_and_include_variables("circular_4.yaml");
  assert!(matches!(result, Err(ParserError::MultipleInclude(_))));
}

#[test]
fn test_empty_list() {
  let result = load_yaml_and_include_variables("empty_list.yaml");
  assert!(matches!(result, Err(ParserError::EmptyList)));
}