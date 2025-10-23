use crate::core::{database::models::Job, jobs::SchedulerTrait};

use super::{Cluster, Config, JobError};

#[derive(Debug, PartialEq, Default)]
pub struct VirtualScheduler;

impl SchedulerTrait for VirtualScheduler {
  fn create_job_script(
    &self,
    job: &Job,
    config: &Config,
    cluster: &Cluster,
  ) -> Result<String, JobError> {
    // FIXME implement virtual job script creation logic
    Ok(String::new())
  }

  fn launch_job(&self, job_script: &str) -> Result<(), JobError> {
    // FIXME implement virtual job launch logic
    Ok(())
  }
}
