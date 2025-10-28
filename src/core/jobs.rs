mod local;
mod pbs;
mod slurm;
mod utils;
mod r#virtual;
use std::io::Write;
use std::{
  fs,
  path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

use serde_json::Value;
use thiserror::Error;

use crate::core::jobs::utils::{escape_for_printf, get_timestamp_string};
use crate::core::{
  cluster_configs::ClusterConfig,
  database::{
    Database,
    models::{Cluster, Config, Job, NewJob, Status},
  },
  jobs::utils::{JobLog, map_err_adding_description, serialize_log_entry},
  parsers::ParsedJob,
};

trait SchedulerTrait {
  fn create_job_script(
    &self,
    job: &Job,
    cluster_config: &ClusterConfig,
  ) -> Result<String, JobError>;
  fn launch_job(&self, job: &mut Job, cluster_config: &ClusterConfig) -> Result<(), JobError>;
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
  ConfigError(#[from] crate::core::sbatchman_configs::SbatchmanConfigError),
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
  #[error("Job Execution: {0}")]
  ExecutionFailed(String),
  #[error("Generic Error: {0}")]
  Other(String),
}

impl Job {
  /// Add preprocessing, main command, and postprocessing to script
  /// This is used by all schedulers to construct the job execution flow
  pub fn add_job_commands(&self, script: &mut String) {
    // Add preprocessing if present
    if let Some(preprocess) = &self.preprocess {
      if !preprocess.is_empty() {
        script.push_str("\n# Preprocessing\n");
        script.push_str(preprocess);
        script.push_str("\n\n");
      }
    }

    // Add the main command
    script.push_str("\n# Main command\n");
    script.push_str(&self.command);
    script.push_str("\n\nSBM_EXIT_CODE=$?");

    // Add postprocessing if present
    if let Some(postprocess) = &self.postprocess {
      if !postprocess.is_empty() {
        script.push_str("\n# Postprocessing\n");
        script.push_str(postprocess);
        script.push_str("\n");
      }
    }
  }

  pub fn get_log_path(&self) -> PathBuf {
    Path::new(&self.directory).join("log.jsonb")
  }
  pub fn get_log(&self) -> std::io::Result<String> {
    fs::read_to_string(self.get_log_path())
  }

  pub fn get_script_path(&self) -> PathBuf {
    Path::new(&self.directory).join("job.sh")
  }
  pub fn get_script(&self) -> std::io::Result<String> {
    fs::read_to_string(self.get_script_path())
  }

  pub fn get_stdout_path(&self) -> PathBuf {
    Path::new(&self.directory).join("stdout.log")
  }
  pub fn get_stdout(&self) -> std::io::Result<String> {
    fs::read_to_string(self.get_stdout_path())
  }

  pub fn get_stderr_path(&self) -> PathBuf {
    Path::new(&self.directory).join("stderr.log")
  }
  pub fn get_stderr(&self) -> std::io::Result<String> {
    fs::read_to_string(self.get_stderr_path())
  }

  /// Ensure job directory exists and return paths for script and log files
  /// This is used by all schedulers to prepare the job directory
  pub fn prepare_job_directory(&self) -> Result<(), JobError> {
    std::fs::create_dir_all(&self.directory)
      .map_err(|e| map_err_adding_description(e, "Could not prepare Job directory {}"))?;
    Ok(())
  }

  fn read_log_entries(&self) -> Result<Vec<serde_json::Value>, std::io::Error> {
    let content = self.get_log()?;
    let entries: Vec<serde_json::Value> = content
      .lines()
      .filter(|line| !line.is_empty())
      .map(|line| serde_json::from_str(line).unwrap())
      .collect();
    Ok(entries)
  }

  /// Write a log entry to the job log file
/// This logs complete job metadata with timestamps for database reconstruction
pub fn write_log_entry(
    &self,
    log: JobLog,
    additional_data: Option<serde_json::Value>,
) -> Result<(), JobError> {
    let mut log_entry = serialize_log_entry(log, additional_data);
    
    // Replace the placeholder with actual timestamp
    if let Some(obj) = log_entry.as_object_mut() {
        obj.insert(
            "timestamp".to_string(),
            Value::String(get_timestamp_string()),
        );
    }

    // Append to log file
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(self.get_log_path())?;

    writeln!(file, "{}", serde_json::to_string(&log_entry).unwrap())?;

    Ok(())
}

/// Creates a bash command to add a log entry to the job log file
/// This logs complete job metadata with timestamps for database reconstruction
pub fn add_log_command(&self, script: &mut String, log: JobLog, additional_data: Option<Value>) {
    let job_log_path = self.get_log_path();
    let abs_path: PathBuf = if job_log_path.is_absolute() {
        job_log_path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("Failed to get current dir")
            .join(job_log_path)
            .canonicalize()
            .expect("Failed to canonicalize path")
    };

    let log_entry = serialize_log_entry(log, additional_data);
    let json_str = serde_json::to_string(&log_entry).unwrap();

    // The placeholder as it appears in the JSON string (with quotes)
    let placeholder_quoted = "\"__TIMESTAMP__\"";
    
    // Find and split at the placeholder
    let pos = json_str.find(placeholder_quoted)
        .expect("Timestamp placeholder not found in JSON");
    
    let before = &json_str[..pos];
    let after = &json_str[pos + placeholder_quoted.len()..];

    // Escape for printf, preserving ${...} for bash variable expansion
    let before_escaped = escape_for_printf(before);
    let after_escaped = escape_for_printf(after);
    
    // Build the printf command
    // The timestamp must be outside quotes to be evaluated
    let printf_cmd = format!(
        "printf '%s\"%s\"%s\\n' '{}' \"$(date +\"%Y-%m-%d %H:%M:%S.%3N\")\" '{}'",
        before_escaped,
        after_escaped
    );

    script.push_str(&format!("\n{} >> {}\n", printf_cmd, abs_path.display()));
}
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
    let launch_result = get_scheduler(&cluster.scheduler).launch_job(
      &mut job,
      &ClusterConfig {
        cluster: cluster,
        config: config,
      },
    );

    if launch_result.is_err() {
      db.update_job_status(job.id, &Status::FailedSubmission)?;
      return Err(JobError::LaunchError("Failed to launch job".to_string()));
    } else {
      // TODO update DB Job (other fields like timestamps, exit_code etc.)
      db.update_job_status(job.id, &job.status)?;
    }
  } else {
    let _ = &r#virtual::VirtualScheduler.launch_job(
      &mut job,
      &ClusterConfig {
        cluster: cluster,
        config: config,
      },
    );
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

fn get_scheduler(scheduler: &DbScheduler) -> Box<dyn SchedulerTrait> {
  match scheduler {
    DbScheduler::Slurm => Box::new(slurm::SlurmScheduler),
    DbScheduler::Pbs => Box::new(pbs::PbsScheduler),
    DbScheduler::Local => Box::new(local::LocalScheduler::default()),
  }
}
