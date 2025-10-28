use chrono::{DateTime, Utc};
use serde_json::json;

use crate::core::cluster_configs::ClusterConfig;
use crate::core::database::models::Status;
use crate::core::jobs::utils::*;
use crate::core::{database::models::Job, jobs::SchedulerTrait};

use super::JobError;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

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

  pub fn new(launch_base_path: PathBuf) -> Self {
    Self {
      launch_base_path: launch_base_path
    }
  }

  /// Submit a job locally with optional timeout
/// Returns (pid, exit_code, timed_out)
fn local_submit(
    &self,
    job: &Job,
    time_limit: Option<&str>,
) -> Result<(u32, Option<i32>, bool), JobError> {
    let stdout_file = File::create(job.get_stdout_path())
        .map_err(|e| map_err_adding_description(e, "Failed to create stdout log: {}"))?;
    let stderr_file = File::create(job.get_stderr_path())
        .map_err(|e| map_err_adding_description(e, "Failed to create stderr log: {}"))?;

    let script_path = job.get_script_path();
    ensure_executable(&script_path)?;

    // Prepare the command (with or without timeout)
    let mut cmd = if let Some(time_str) = time_limit {
        let timeout_seconds = parse_time_to_seconds(time_str)?;
        let mut c = Command::new("timeout");
        c.arg(timeout_seconds.to_string()).arg(script_path);
        c
    } else {
        Command::new(script_path)
    };

    cmd.stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    // println!("CMD {:#?}", cmd);

    // Run the command
    let mut child = cmd
        .spawn()
        .map_err(|e| JobError::SpawnError(format!("Failed to spawn process: {}", e)))?;

      job.write_log_entry(JobLog::StatusUpdate(Status::Running), None)?;
    let pid = child.id();

    let output = child
        .wait()
        .map_err(|e| JobError::WaitError(format!("Failed to wait for process: {}", e)))?;

    let exit_code = output.code();
        // println!("sstsus {:#?}", output);
        // println!("succc {:#?}", output.success());
        // println!("stdout:{:#?}", job.get_stdout());
        // println!("stderr:{:#?}", job.get_stderr());
    let timed_out = exit_code == Some(124); // "timeout" command exit code

        // println!("SCRIPT\n{}", job.get_script()?);
        // println!("CODE {:?}", exit_code);
        // println!("LOG {:?}", job.get_log()?);

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

    script.push_str("exit \"${SBM_EXIT_CODE}\"");

    Ok(script)
  }

  fn launch_job(&self, job: &mut Job, cluster_config: &ClusterConfig) -> Result<(), JobError> {
    job.prepare_job_directory()?;
    job.write_log_entry(JobLog::Metadata(job.clone()), None)?;

    // Create the job script
    let script_path = job.get_script_path();
    let script_content = self.create_job_script(job, cluster_config)?;

    // Save script to job directory
    {// FIXME this seems to be an issue sometimes SpawnError("Failed to spawn process: Text file busy (os error 26)")
    let mut file = File::create(&script_path)
      .map_err(|e| map_err_adding_description(e, "Failed to create script file: {}"))?;

    file
      .write_all(script_content.as_bytes())
      .map_err(|e| map_err_adding_description(e, "Failed to write script: {}"))?;

    // Explicitly flush and close the file
    file.flush()
        .map_err(|e| map_err_adding_description(e, "Failed to flush script: {}"))?;
      
    } // File is dropped and closed here

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
      JobLog::StatusUpdate(status.clone()),
      json!({"pid": pid}).into(),
    )?;
    match status {
        Status::FailedSubmission => Err(JobError::ExecutionFailed("Could not run job".to_string())),
        _ => Ok(())
    }
  }

  fn get_number_of_enqueued_jobs(&self) -> Result<usize, JobError> {
    // For local scheduler, there's no queue - jobs run immediately
    Ok(0)
  }
}
