use std::path::Path;

use crate::core::{cluster_configs::ClusterConfig, database::models::Job, jobs::SchedulerTrait};

use super::JobError;

pub struct PbsScheduler;

impl SchedulerTrait for PbsScheduler {
  fn create_job_script(
    &self,
    job: &Job,
    cluster_config: &ClusterConfig,
  ) -> Result<String, JobError> {
    // FIXME implement PBS job script creation logic
    Ok(String::new())
  }

  fn launch_job(&self, job: &mut Job, cluster_config: &ClusterConfig) -> Result<(), JobError> {
    Ok(())
  }

  fn get_number_of_enqueued_jobs(&self) -> Result<usize, JobError> {
    // FIXME implement logic to get number of enqueued jobs
    Ok(0)
  }
}
