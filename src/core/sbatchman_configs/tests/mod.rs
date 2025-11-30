use crate::core::{sbatchman_configs::{
  get_sbatchman_config_global, get_sbatchman_config_local, init_sbatchman_config_local, set_sbatchman_config_global, set_sbatchman_config_local
}};

#[test]
fn sbatchman_init_test() {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(init_sbatchman_config_local(&path).is_ok());
  let config_path = path.join("sbatchman.conf");
  assert!(config_path.exists());
}

pub fn init_sbatchman_for_tests() -> tempfile::TempDir {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(init_sbatchman_config_local(&path).is_ok());
  temp_dir
}

#[test]
fn set_and_get_cluster_name_test_local() {
  let temp_dir = init_sbatchman_for_tests();
  let mut config = get_sbatchman_config_local(&temp_dir.path().to_path_buf()).unwrap();
  config.cluster_name = Some("test_cluster_local".to_string());
  assert!(set_sbatchman_config_local(&temp_dir.path().to_path_buf(), &config).is_ok());
  assert_eq!(get_sbatchman_config_local(&temp_dir.path().to_path_buf()).expect("No config file found").cluster_name.expect("No cluster name found"), "test_cluster_local");
}

#[test]
fn set_and_get_cluster_name_test_global() {
  let mut config = get_sbatchman_config_global().unwrap();
  config.cluster_name = Some("test_cluster_global".to_string());
  assert!(set_sbatchman_config_global(&config).is_ok());
  assert_eq!(get_sbatchman_config_global().expect("No config file found").cluster_name.expect("No cluster name found"), "test_cluster_global");
}



