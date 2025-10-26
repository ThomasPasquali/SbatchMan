use std::path::Path;

use crate::core::{cluster_configs::ClusterConfig, database::models::Job, jobs::SchedulerTrait};

use super::JobError;

#[derive(Debug, PartialEq, Default)]
pub struct VirtualScheduler;

impl SchedulerTrait for VirtualScheduler {
  fn create_job_script(
    &self,
    script_path: &Path,
    job: &Job,
    cluster_config: &ClusterConfig,
  ) -> Result<String, JobError> {
    // FIXME implement virtual job script creation logic
    Ok(String::new())
  }

  fn launch_job(&self, job: &mut Job, cluster_config: &ClusterConfig) -> Result<(), JobError> {
    Ok(())
  }
}
