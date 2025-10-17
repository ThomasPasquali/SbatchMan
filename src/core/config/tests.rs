use crate::core::config::{get_cluster_name, sbatchman_init, set_cluster_name};

#[test]
fn sbatchman_init_test() {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(sbatchman_init(&path).is_ok());
  let config_path = path.join("sbatchman.conf");
  assert!(config_path.exists());
}

pub fn init_sbatchman_for_tests() -> tempfile::TempDir {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(sbatchman_init(&path).is_ok());
  temp_dir
}

#[test]
fn set_and_get_cluster_name_test() {
  let temp_dir = init_sbatchman_for_tests();
  let cluster_name = "test_cluster";
  assert!(set_cluster_name(&temp_dir.path().to_path_buf(), cluster_name).is_ok());
  let retrieved_name = get_cluster_name(&temp_dir.path().to_path_buf()).unwrap();
  assert_eq!(retrieved_name, cluster_name);
}
