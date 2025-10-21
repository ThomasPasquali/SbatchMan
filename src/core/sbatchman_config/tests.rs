use crate::core::sbatchman_config::{init_sbatchman_config, set_sbatchman_config, get_sbatchman_config};

#[test]
fn sbatchman_init_test() {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(init_sbatchman_config(&path).is_ok());
  let config_path = path.join("sbatchman.conf");
  assert!(config_path.exists());
}

pub fn init_sbatchman_for_tests() -> tempfile::TempDir {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(init_sbatchman_config(&path).is_ok());
  temp_dir
}

#[test]
fn set_and_get_cluster_name_test() {
  let temp_dir = init_sbatchman_for_tests();
  let mut config = get_sbatchman_config(&temp_dir.path().to_path_buf()).unwrap();
  config.cluster_name = Some("test_cluster".to_string());
  assert!(set_sbatchman_config(&temp_dir.path().to_path_buf(), &config).is_ok());
}
