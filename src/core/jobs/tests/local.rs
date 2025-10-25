// ============================================================================
// "Manual" tests
// ============================================================================

use crate::core::jobs::{
  SchedulerTrait,
  local::LocalScheduler,
  tests::{create_test_cluster, create_test_config, create_test_job},
};

#[test]
fn test_job_launch() {
  let path = "./test_job";
  let mut job = create_test_job(1, path);
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let local_scheduler = LocalScheduler::default();

  assert!(
    local_scheduler
      .launch_job(&mut job, &config, &cluster)
      .is_ok()
  );
}
