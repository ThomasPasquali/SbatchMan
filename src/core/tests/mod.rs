use crate::core::storage::*;

#[test]
fn sbatchman_init_test() {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(sbatchman_init(&path).is_ok());
  let config_path = path.join("sbatchman.conf");
  assert!(config_path.exists());
}

#[test]
fn set_and_get_cluster_name_test() {
  let temp_dir = tempfile::tempdir().unwrap();
  let path = temp_dir.path().to_path_buf();
  assert!(sbatchman_init(&path).is_ok());
  let cluster_name = "test_cluster";
  assert!(set_cluster_name(&path, cluster_name).is_ok());
  let retrieved_name = get_cluster_name(&path).unwrap();
  assert_eq!(retrieved_name, cluster_name);
}