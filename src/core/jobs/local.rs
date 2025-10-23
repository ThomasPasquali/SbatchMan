use serde_json::json;

use crate::core::jobs::utils::*;
use crate::core::{database::models::Job, jobs::SchedulerTrait};

use super::{Cluster, Config, JobError};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Default)]
pub struct LocalScheduler;

impl LocalScheduler {
  /// Submit a job locally with optional timeout
  /// Returns (pid, timed_out)
  fn local_submit(
    &self,
    script_path: &Path,
    job_dir: &Path,
    log_path: &Path,
    time_limit: Option<&str>,
    job: &Job,
    config: &Config,
    cluster: &Cluster,
  ) -> Result<(u32, bool), JobError> {
    let stdout_log = job_dir.join("stdout.log");
    let stderr_log = job_dir.join("stderr.log");

    // Log job start
    write_log_entry(
      log_path,
      "job_execution_start",
      job,
      config,
      cluster,
      Some(json!({
        "script_path": script_path.to_str(),
        "time_limit": time_limit,
      })),
    )?;

    let start_time = get_timestamp();

    let stdout_file = File::create(&stdout_log).map_err(|e| {
      JobError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("Failed to create stdout log: {}", e),
      ))
    })?;
    let stderr_file = File::create(&stderr_log).map_err(|e| {
      JobError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("Failed to create stderr log: {}", e),
      ))
    })?;

    let mut timed_out = false;
    let pid: u32;
    let exit_code: Option<i32>;

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

      // Log process spawned
      write_log_entry(
        log_path,
        "process_spawned",
        job,
        config,
        cluster,
        Some(json!({
          "pid": pid,
          "with_timeout": true,
          "timeout_seconds": timeout_seconds,
        })),
      )?;

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

      // Log process spawned
      write_log_entry(
        log_path,
        "process_spawned",
        job,
        config,
        cluster,
        Some(json!({
          "pid": pid,
          "with_timeout": false,
        })),
      )?;

      let output = child
        .wait_with_output()
        .map_err(|e| JobError::WaitError(format!("Failed to wait for process: {}", e)))?;

      exit_code = output.status.code();
    }

    let end_time = get_timestamp();
    let duration_ms = (end_time - start_time).num_milliseconds();

    // Log job completion
    write_log_entry(
      log_path,
      "job_execution_end",
      job,
      config,
      cluster,
      Some(json!({
        "pid": pid,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "duration_ms": duration_ms,
        "start_time": start_time.to_rfc3339(),
        "end_time": end_time.to_rfc3339(),
      })),
    )?;

    Ok((pid, timed_out))
  }
}

impl SchedulerTrait for LocalScheduler {
  fn create_job_script(
    &self,
    job: &Job,
    config: &Config,
    cluster: &Cluster,
  ) -> Result<String, JobError> {
    let job_dir = PathBuf::from(&job.directory);
    let (_, log_path) = prepare_job_directory(&job_dir)?;

    // Log job script creation
    write_log_entry(
      &log_path,
      "job_script_creation_start",
      job,
      config,
      cluster,
      None,
    )?;

    let mut script = String::new();

    // Add script header
    script.push_str(&generate_script_header(job, config, cluster));
    script.push_str("# Local execution script\n\n");

    // Add environment variables
    add_environment_variables(&mut script, config);

    // Add job commands (preprocess, main, postprocess)
    add_job_commands(&mut script, job);

    // Log job script created
    write_log_entry(
      &log_path,
      "job_script_created",
      job,
      config,
      cluster,
      Some(json!({
        "script_length": script.len(),
      })),
    )?;

    Ok(script)
  }

  fn launch_job(&self, job_script: &str) -> Result<(), JobError> {
    // Note: This method signature doesn't provide job/config/cluster info
    // You may want to modify the SchedulerTrait to include these parameters
    // For now, we'll create a basic implementation

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join(format!("job_{}.sh", std::process::id()));

    let mut file = File::create(&script_path).map_err(|e| {
      JobError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("Failed to create script file: {}", e),
      ))
    })?;

    file.write_all(job_script.as_bytes()).map_err(|e| {
      JobError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("Failed to write script: {}", e),
      ))
    })?;

    // Make script executable
    make_script_executable(&script_path)?;

    // Launch the job
    let mut command = Command::new("bash");
    command.arg(&script_path);

    let child = command
      .spawn()
      .map_err(|e| JobError::SpawnError(format!("Failed to spawn process: {}", e)))?;

    let output = child
      .wait_with_output()
      .map_err(|e| JobError::WaitError(format!("Failed to wait for process: {}", e)))?;

    // Clean up temporary script
    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
      return Err(JobError::ExecutionFailed(format!(
        "Job failed with exit code: {:?}",
        output.status.code()
      )));
    }

    Ok(())
  }

  fn get_number_of_enqueued_jobs(&self) -> Result<usize, JobError> {
    // For local scheduler, there's no queue - jobs run immediately
    Ok(0)
  }
}

// Additional method that should be added to SchedulerTrait or called separately
impl LocalScheduler {
  /// Launch a job with full context for proper logging
  pub fn launch_job_with_context(
    &self,
    job: &Job,
    config: &Config,
    cluster: &Cluster,
  ) -> Result<(), JobError> {
    let job_dir = PathBuf::from(&job.directory);
    let (script_path, log_path) = prepare_job_directory(&job_dir)?;

    // Log job submission
    write_log_entry(
      &log_path,
      "job_submission",
      job,
      config,
      cluster,
      Some(json!({
        "submission_time": get_timestamp().to_rfc3339(),
      })),
    )?;

    // Create the job script
    let script_content = self.create_job_script(job, config, cluster)?;

    // Save script to job directory
    let mut file = File::create(&script_path).map_err(|e| {
      JobError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("Failed to create script file: {}", e),
      ))
    })?;

    file.write_all(script_content.as_bytes()).map_err(|e| {
      JobError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("Failed to write script: {}", e),
      ))
    })?;

    // Log script saved
    write_log_entry(
      &log_path,
      "job_script_saved",
      job,
      config,
      cluster,
      Some(json!({
        "script_path": script_path.to_str(),
      })),
    )?;

    // Make script executable
    make_script_executable(&script_path)?;

    // Extract time limit from config flags if present
    let time_limit = config.flags.get("time").and_then(|v| v.as_str());

    // Launch the job with full logging
    let (pid, timed_out) = self.local_submit(
      &script_path,
      &job_dir,
      &log_path,
      time_limit,
      job,
      config,
      cluster,
    )?;

    if timed_out {
      write_log_entry(
        &log_path,
        "job_timeout",
        job,
        config,
        cluster,
        Some(json!({
          "pid": pid,
        })),
      )?;
      return Err(JobError::Timeout(format!("Job timed out (PID: {})", pid)));
    }

    // Log final success
    write_log_entry(
      &log_path,
      "job_completed_successfully",
      job,
      config,
      cluster,
      Some(json!({
        "pid": pid,
      })),
    )?;

    Ok(())
  }
}
