use crate::core::{
  database::{models::*, *},
  sbatchman_configs::tests::init_sbatchman_for_tests,
};

#[test]
fn get_set_config() {
  let dir = init_sbatchman_for_tests();
  let mut db = Database::new(&dir.path().to_path_buf()).unwrap();
  let new_cluster = NewCluster {
    cluster_name: "test_cluster".to_string(),
    scheduler: Scheduler::Local,
    max_jobs: Some(10),
  };
  let cluster = db.create_cluster(&new_cluster).unwrap();

  let flags = serde_json::json!({"flag1": "value1", "flag2": "value2"});
  let env = serde_json::json!({"env1": "value1", "env2": "value2"});

  let new_config = NewConfig {
    config_name: "test_config".to_string(),
    cluster_id: cluster.id,
    flags: flags,
    env: env,
  };
  db.create_cluster_config(&new_config).unwrap();
  let configs = db.get_configs_by_cluster(&cluster).unwrap();
  assert!(configs.contains_key("test_config"));
}

#[test]
fn create_cluster_same_name() {
  let dir = init_sbatchman_for_tests();
  let mut db = Database::new(&dir.path().to_path_buf()).unwrap();

  let new_cluster = NewCluster {
    cluster_name: "duplicate_cluster".to_string(),
    scheduler: Scheduler::Local,
    max_jobs: Some(10),
  };
  let _cluster1 = db.create_cluster(&new_cluster).unwrap();
  let result = db.create_cluster(&new_cluster);
  assert!(result.is_err());
}
