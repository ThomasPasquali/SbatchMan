use crate::core::{database::models::Job, jobs::SchedulerTrait};

use super::{Cluster, Config, JobError};

pub struct PbsScheduler;

impl SchedulerTrait for PbsScheduler {
  fn create_job_script(
    &self,
    job: &Job,
    config: &Config,
    cluster: &Cluster,
  ) -> Result<String, JobError> {
    // FIXME implement PBS job script creation logic
    Ok(String::new())
  }

  fn launch_job(&self, job_script: &str) -> Result<(), JobError> {
    // FIXME implement PBS job launch logic
    Ok(())
  }

  fn get_number_of_enqueued_jobs(&self) -> Result<usize, JobError> {
    // FIXME implement logic to get number of enqueued jobs
    Ok(0)
  }
}
