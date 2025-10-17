use std::path::Path;

use saphyr::Yaml;

use crate::core::parsers::{load_and_merge_includes, utils::{load_yaml_from_file, yaml_lookup}};

#[test]
fn merge_includes() {
  unsafe { std::env::set_var("RUST_LOG", "debug") };
  env_logger::init();

  let src_dir = Path::new(file!()).parent().unwrap();
  let yaml =
    Box::new(load_yaml_from_file(&src_dir.join("files").join("clusters_configs.yaml")).unwrap());

  let yaml = load_and_merge_includes(yaml, &src_dir.join("files")).unwrap();
  assert_eq!(yaml_lookup(&yaml, "include"), None);
  
  let vars = yaml_lookup(&yaml, "variables").unwrap();
  // println!("{:#?}", vars.as_mapping().unwrap().keys());
  // Maps
  for k in vec!["partition", "qos", "args", "nodes"] {
    match yaml_lookup(vars, k) {
      Some(Yaml::Mapping(_)) => {},
      _ => panic!("Key {} not found!", k)
    }
  }
  // Sequences
  for k in vec!["implementation", "task_cpus"] {
    match yaml_lookup(vars, k) {
      Some(Yaml::Mapping(_)) => {},
      _ => panic!("Key {} not found!", k)
    }
  }
  // Values
  for k in vec!["dataset", "mode", "another_var", "to_override"] {
    match yaml_lookup(vars, k) {
      Some(Yaml::Mapping(_)) => {},
      _ => panic!("Key {} not found!", k)
    }
  }
}
