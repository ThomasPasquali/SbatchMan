use chrono::{DateTime, Utc};
use serde_json::json;

use crate::core::cluster_configs::ClusterConfig;
use crate::core::database::models::Status;
use crate::core::jobs::utils::*;
use crate::core::{database::models::Job, jobs::SchedulerTrait};

use super::JobError;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq)]
pub struct LocalScheduler {
  pub launch_base_path: PathBuf,
}

impl Default for LocalScheduler {
  fn default() -> Self {
    Self {
      launch_base_path: PathBuf::from("."),
    }
  }
}

impl LocalScheduler {
  /// Submit a job locally with optional timeout
  /// Returns (pid, exit_code, timed_out)
  fn local_submit(
    &self,
    job: &Job,
    time_limit: Option<&str>,
  ) -> Result<(u32, Option<i32>, bool), JobError> {
    let stdout_log = job.get_stdout_path();
    let stderr_log = job.get_stderr_path();
    let script_path = job.get_script_path();

    let stdout_file = File::create(&stdout_log)
      .map_err(|e| map_err_adding_description(e, "Failed to create stdout log: {}"))?;
    let stderr_file = File::create(&stderr_log)
      .map_err(|e| map_err_adding_description(e, "Failed to create stderr log: {}"))?;

    let mut timed_out = false;
    let pid: u32;
    let exit_code: Option<i32>;
    let mut start_time: Option<DateTime<Utc>> = None;

    if let Some(time_str) = time_limit {
      // Use timeout command if time limit is specified
      let timeout_seconds = parse_time_to_seconds(time_str)?;

      let mut timeout_cmd = Command::new("timeout");
      timeout_cmd
        .arg(timeout_seconds.to_string())
        .arg("bash")
        .arg(script_path.to_str().unwrap())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

      let child = timeout_cmd
        .spawn()
        .map_err(|e| JobError::SpawnError(format!("Failed to spawn process: {}", e)))?;

      pid = child.id();
      start_time = Some(get_timestamp());
      job.write_log_entry(JobLog::StatusUpdate(Status::Running), None)?;

      let output = child
        .wait_with_output()
        .map_err(|e| JobError::WaitError(format!("Failed to wait for process: {}", e)))?;

      exit_code = output.status.code();

      // Exit code 124 indicates timeout
      if exit_code == Some(124) {
        timed_out = true;
      }
    } else {
      // No timeout, run directly
      let mut command = Command::new("bash");
      command
        .arg(script_path.to_str().unwrap())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

      let child = command
        .spawn()
        .map_err(|e| JobError::SpawnError(format!("Failed to spawn process: {}", e)))?;

      pid = child.id();
      start_time = Some(get_timestamp());
      job.write_log_entry(JobLog::StatusUpdate(Status::Running), None)?;

      let output = child
        .wait_with_output()
        .map_err(|e| JobError::WaitError(format!("Failed to wait for process: {}", e)))?;

      exit_code = output.status.code();
    }

    Ok((pid, exit_code, timed_out))
  }
}

impl SchedulerTrait for LocalScheduler {
  fn create_job_script(
    &self,
    job: &Job,
    cluster_config: &ClusterConfig,
  ) -> Result<String, JobError> {
    let mut script = cluster_config.generate_script_header(&self.launch_base_path);

    cluster_config.add_environment_variables(&mut script);

    job.add_job_commands(&mut script);

    job.add_log_command(
      &mut script,
      JobLog::BashVariable("SBM_EXIT_CODE".to_string()),
      None,
    );

    Ok(script)
  }

  fn launch_job(&self, job: &mut Job, cluster_config: &ClusterConfig) -> Result<(), JobError> {
    job.prepare_job_directory()?;
    job.write_log_entry(JobLog::Metadata(job.clone()), None)?;

    // Create the job script
    let script_path = job.get_script_path();
    let script_content = self.create_job_script(job, cluster_config)?;

    // Save script to job directory
    let mut file = File::create(&script_path)
      .map_err(|e| map_err_adding_description(e, "Failed to create script file: {}"))?;

    file
      .write_all(script_content.as_bytes())
      .map_err(|e| map_err_adding_description(e, "Failed to write script: {}"))?;

    // Make script executable
    make_script_executable(&script_path)?;

    job.write_log_entry(JobLog::StatusUpdate(Status::Created), None)?;

    // Extract time limit from config flags if present
    let time_limit = cluster_config
      .config
      .flags
      .get("time")
      .and_then(|v| v.as_str());

    // Launch the job with full logging
    let (pid, exit_code, timed_out) = self.local_submit(job, time_limit)?;

    // FIXME should be done better
    let status = if timed_out {
      Status::Timeout
    } else if let Some(code) = exit_code {
      if code == 0 {
        Status::Completed
      } else {
        Status::Failed
      }
    } else {
      Status::FailedSubmission // FIXME Especially this
    };
    job.write_log_entry(
      JobLog::StatusUpdate(status),
      json!({"pid": pid}).into(),
    )?;

    Ok(())
  }

  fn get_number_of_enqueued_jobs(&self) -> Result<usize, JobError> {
    // For local scheduler, there's no queue - jobs run immediately
    Ok(0)
  }
}
