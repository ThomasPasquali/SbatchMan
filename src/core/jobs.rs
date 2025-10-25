mod local;
mod pbs;
mod slurm;
mod utils;
mod r#virtual;
use std::path::PathBuf;

#[cfg(test)]
mod tests;

use thiserror::Error;

use crate::core::{
  database::{
    Database,
    models::{Cluster, Config, Job, NewJob, Status},
  },
  parsers::ParsedJob,
};

trait SchedulerTrait {
  fn create_job_script(
    &self,
    job: &Job,
    config: &Config,
    cluster: &Cluster,
  ) -> Result<String, JobError>;
  fn launch_job(&self, job: &mut Job, config: &Config, cluster: &Cluster) -> Result<(), JobError>;
  fn get_number_of_enqueued_jobs(&self) -> Result<usize, JobError> {
    Ok(0)
  }
}

use crate::core::database::models::Scheduler as DbScheduler;

#[derive(Error, Debug)]
pub enum JobError {
  #[error("Launch Error: {0}")]
  LaunchError(String),
  #[error("Parser Error: {0}")]
  ParserError(#[from] crate::core::parsers::ParserError),
  #[error("Config Error: {0}")]
  ConfigError(#[from] crate::core::sbatchman_config::SbatchmanConfigError),
  #[error("Database Error: {0}")]
  DatabaseError(#[from] crate::core::database::StorageError),
  #[error("Config '{0}' not found for cluster")]
  ConfigNotFound(String),
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("Invalid Time Format: {0}")]
  InvalidTimeFormat(String),
  #[error("Job Spawn: {0}")]
  SpawnError(String),
  #[error("Job Wait: {0}")]
  WaitError(String),
  #[error("Job Timeout: {0}")]
  Timeout(String),
  #[error("Job Execution: {0}")]
  ExecutionFailed(String),
}

pub fn launch_jobs_from_file(
  path: &PathBuf,
  db: &mut Database,
  cluster_name: &str,
) -> Result<(), JobError> {
  let jobs = crate::core::parsers::parse_jobs_from_file(path)?;
  let cluster = db.get_cluster_by_name(cluster_name)?;
  let configs = db.get_configs_by_cluster(&cluster)?;
  let mut to_launch_really = jobs.len();
  if let Some(max_jobs) = cluster.max_jobs {
    let enqueued_jobs = get_scheduler(&cluster.scheduler).get_number_of_enqueued_jobs()?;
    // Number of jobs that can be enqueued without exceeding max_jobs
    to_launch_really = std::cmp::min(
      to_launch_really,
      (max_jobs as usize).saturating_sub(enqueued_jobs),
    );
  }
  let mut iter = jobs.iter();
  // Launch jobs up to the allowed limit
  while to_launch_really > 0 {
    let job = iter.next().unwrap();
    let config = configs
      .get(job.config_name)
      .ok_or(JobError::ConfigNotFound(job.config_name.to_string()))?;
    launch_job(job, config, &cluster, db, path, false)?;
    to_launch_really -= 1;
  }
  // Remaining jobs go to virtual queue
  while let Some(job) = iter.next() {
    let config = configs
      .get(job.config_name)
      .ok_or(JobError::ConfigNotFound(job.config_name.to_string()))?;
    launch_job(job, config, &cluster, db, path, true)?;
  }

  return Ok(());
}

fn launch_job(
  job: &ParsedJob,
  config: &Config,
  cluster: &Cluster,
  db: &mut Database,
  path: &PathBuf,
  virtual_queue: bool,
) -> Result<(), JobError> {
  let new_job = NewJob {
    job_name: job.job_name,
    command: job.command,
    preprocess: job.preprocess,
    postprocess: job.postprocess,
    variables: job.variables,
    config_id: config.id,
    status: &Status::Created,
    directory: "",
  };

  let mut job = db.create_job(&new_job)?;
  // Set directory name to ID assigned by the database
  let path = create_job_dir(path, job.id)?;
  db.update_job_path(job.id, path.to_str().unwrap())?;

  // let script = get_scheduler(&cluster.scheduler).create_job_script(&job, config, cluster);
  if !virtual_queue {
    // FIXME: Should we update the submit time here or in the job script?
    let launch_result = get_scheduler(&cluster.scheduler).launch_job(&mut job, config, cluster);

    if launch_result.is_err() {
      db.update_job_status(job.id, &Status::FailedSubmission)?;
      return Err(JobError::LaunchError("Failed to launch job".to_string()));
    } else {
      // TODO update DB Job (other fields like timestamps, exit_code etc.)
      db.update_job_status(job.id, &job.status)?;
    }
  } else {
    let _ = &r#virtual::VirtualScheduler.launch_job(&mut job, config, cluster);
    db.update_job_status(job.id, &Status::VirtualQueue)?;
  }
  Ok(())
}

fn create_job_dir(path: &PathBuf, id: i32) -> Result<PathBuf, JobError> {
  use std::fs;
  use std::path::Path;

  let dir_path = path.join(format!("jobs/{}", id));
  fs::create_dir_all(Path::new(&dir_path))?;
  Ok(dir_path)
}

fn get_scheduler(scheduler: &DbScheduler) -> &dyn SchedulerTrait {
  match scheduler {
    DbScheduler::Slurm => &slurm::SlurmScheduler,
    DbScheduler::Pbs => &pbs::PbsScheduler,
    DbScheduler::Local => &local::LocalScheduler,
  }
}
