use chrono::{DateTime, Utc};
use serde_json::json;
use std::fs::create_dir_all;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::core::{
  database::models::{Cluster, Config, Job},
  jobs::JobError,
};

/// Get current timestamp as DateTime<Utc>
pub fn get_timestamp() -> DateTime<Utc> {
  Utc::now()
}

/// Write a log entry to the job log file
/// This logs complete job metadata with timestamps for database reconstruction
pub fn write_log_entry(
  log_path: &Path,
  event: &str,
  job: &Job,
  config: &Config,
  cluster: &Cluster,
  additional_data: Option<serde_json::Value>,
) -> Result<(), JobError> {
  let timestamp = get_timestamp();

  let mut log_entry = json!({
    "timestamp": timestamp.to_rfc3339(),
    "timestamp_ms": timestamp.timestamp_millis(),
    "event": event,
    "job": {
      "id": job.id,
      "job_name": &job.job_name,
      "config_id": job.config_id,
      "submit_time": job.submit_time,
      "directory": &job.directory,
      "command": &job.command,
      "status": format!("{:?}", job.status),
      "job_id": &job.job_id,
      "end_time": job.end_time,
      "preprocess": &job.preprocess,
      "postprocess": &job.postprocess,
      "archived": job.archived,
      "variables": &job.variables,
    },
    "config": {
      "id": config.id,
      "config_name": &config.config_name,
      "cluster_id": config.cluster_id,
      "flags": &config.flags,
      "env": &config.env,
    },
    "cluster": {
      "id": cluster.id,
      "cluster_name": &cluster.cluster_name,
      "scheduler": format!("{:?}", cluster.scheduler),
      "max_jobs": cluster.max_jobs,
    }
  });

  // Add any additional data
  if let Some(data) = additional_data {
    log_entry
      .as_object_mut()
      .unwrap()
      .insert("additional".to_string(), data);
  }

  // Append to log file
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log_path)?;

  writeln!(file, "{}", serde_json::to_string(&log_entry).unwrap())?;

  Ok(())
}

/// Parse time string in format "HH:MM:SS" or "D-HH:MM:SS" to seconds
/// This is used by SLURM, PBS, and local schedulers for time limits
pub fn parse_time_to_seconds(time_str: &str) -> Result<u64, JobError> {
  if time_str.contains('-') {
    // Format: D-HH:MM:SS
    let parts: Vec<&str> = time_str.split('-').collect();
    if parts.len() != 2 {
      return Err(JobError::InvalidTimeFormat(time_str.to_string()));
    }

    let days: u64 = parts[0]
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;

    let time_parts: Vec<&str> = parts[1].split(':').collect();
    if time_parts.len() != 3 {
      return Err(JobError::InvalidTimeFormat(time_str.to_string()));
    }

    let hours: u64 = time_parts[0]
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;
    let minutes: u64 = time_parts[1]
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;
    let seconds: u64 = time_parts[2]
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;

    Ok(days * 86400 + hours * 3600 + minutes * 60 + seconds)
  } else {
    // Format: HH:MM:SS
    let time_parts: Vec<&str> = time_str.split(':').collect();
    if time_parts.len() != 3 {
      return Err(JobError::InvalidTimeFormat(time_str.to_string()));
    }

    let hours: u64 = time_parts[0]
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;
    let minutes: u64 = time_parts[1]
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;
    let seconds: u64 = time_parts[2]
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;

    Ok(hours * 3600 + minutes * 60 + seconds)
  }
}

/// Ensure job directory exists and return paths for script and log files
/// This is used by all schedulers to prepare the job directory
pub fn prepare_job_directory(job_dir: &Path) -> Result<(PathBuf, PathBuf), JobError> {
  create_dir_all(job_dir)?;

  let script_path = job_dir.join("job.sh");
  let log_path = job_dir.join("job.log");

  Ok((script_path, log_path))
}

/// Generate bash script header with job metadata
/// This is used by all schedulers to create consistent script headers
pub fn generate_script_header(job: &Job, config: &Config, cluster: &Cluster) -> String {
  let mut script = String::new();
  script.push_str("#!/bin/bash\n\n");
  script.push_str(&format!("# Job ID: {}\n", job.id));
  script.push_str(&format!("# Job Name: {}\n", job.job_name));
  script.push_str(&format!("# Config: {}\n", config.config_name));
  script.push_str(&format!("# Cluster: {}\n", cluster.cluster_name));
  script.push_str(&format!("# Scheduler: {:?}\n", cluster.scheduler));
  script.push_str("\n");
  script
}

/// Add environment variables from config to script
/// This is used by all schedulers to set up the job environment
pub fn add_environment_variables(script: &mut String, config: &Config) {
  if let Some(env_obj) = config.env.as_object() {
    if !env_obj.is_empty() {
      script.push_str("# Environment variables\n");
      for (key, value) in env_obj {
        script.push_str(&format!("export {}={}\n", key, value.to_string()));
      }
      script.push_str("\n");
    }
  }
}

/// Add preprocessing, main command, and postprocessing to script
/// This is used by all schedulers to construct the job execution flow
pub fn add_job_commands(script: &mut String, job: &Job) {
  // Add preprocessing if present
  if let Some(preprocess) = &job.preprocess {
    if !preprocess.is_empty() {
      script.push_str("# Preprocessing\n");
      script.push_str(preprocess);
      script.push_str("\n\n");
    }
  }

  // Add the main command
  script.push_str("# Main command\n");
  script.push_str(&job.command);
  script.push_str("\n\n");

  // Add postprocessing if present
  if let Some(postprocess) = &job.postprocess {
    if !postprocess.is_empty() {
      script.push_str("# Postprocessing\n");
      script.push_str(postprocess);
      script.push_str("\n");
    }
  }
}

/// Make a script file executable (Unix only)
#[cfg(unix)]
pub fn make_script_executable(script_path: &Path) -> Result<(), JobError> {
  use std::os::unix::fs::PermissionsExt;
  let metadata = std::fs::metadata(script_path)?;
  let mut perms = metadata.permissions();
  perms.set_mode(0o755);
  std::fs::set_permissions(script_path, perms)?;
  Ok(())
}

#[cfg(not(unix))]
pub fn make_script_executable(_script_path: &Path) -> Result<(), JobError> {
  // FIXME On non-Unix systems, scripts don't need executable permissions
  Ok(())
}
