use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::create_dir_all;
use std::io::{Error, Write};
use std::path::{Path, PathBuf};

use crate::core::database::models::Status;
use crate::core::{
  database::models::{Cluster, Config, Job},
  jobs::JobError,
};

pub fn map_err_adding_description(error: Error, description: &str) -> JobError {
  JobError::IoError(std::io::Error::new(
    std::io::ErrorKind::Other,
    format!("{}: {}", description, error),
  ))
}

/// Get current timestamp as DateTime<Utc>
pub fn get_timestamp() -> DateTime<Utc> {
  Utc::now()
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum JobLog {
  Metadata(Job),
  StatusUpdate(Status),
  StatusUpdateBash(String), // The string contains the bash variable name that contains the status
}

fn serialize_log_entry(log: JobLog, additional_data: Option<serde_json::Value>) -> Value {
  let mut log_entry = json!(log);
  println!("{:#?}", log_entry);

  // Add any additional data
  if let Some(data) = additional_data {
    log_entry
      .as_object_mut()
      .unwrap()
      .insert("additional".to_string(), data);
  }
  log_entry
}

/// Write a log entry to the job log file
/// This logs complete job metadata with timestamps for database reconstruction
pub fn write_log_entry(
  log_path: &Path,
  log: JobLog,
  additional_data: Option<serde_json::Value>,
) -> Result<(), JobError> {
  let log_entry = serialize_log_entry(log, additional_data);

  // Append to log file
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log_path)?;

  writeln!(file, "{}", serde_json::to_string(&log_entry).unwrap())?;

  Ok(())
}

/// Creates a bash command to add a log entry to the job log file
/// This logs complete job metadata with timestamps for database reconstruction
pub fn add_log_command(
  script: &mut String,
  script_path: &Path,
  log: JobLog,
  additional_data: Option<Value>,
) {
  let abs_path: PathBuf = if script_path.is_absolute() {
    script_path.to_path_buf()
  } else {
    std::env::current_dir()
      .expect("Failed to get current dir")
      .join(script_path)
      .canonicalize()
      .expect("Failed to canonicalize path")
  };

  let mut log_entry = serialize_log_entry(log, additional_data);
  log_entry.as_object_mut().unwrap().insert(
    "timestamp".to_string(),
    Value::String("$(date +\"%Y-%m-%d %H:%M:%S.%3N\")".to_string()),
  );

  let json_str = serde_json::to_string(&log_entry).unwrap();

  // Escape all double quotes except those in $(date ...)
  // We first escape all, then restore the ones around the timestamp
  let escaped = json_str.replace('"', "\\\"").replace(
    "\\\"$(date +\\\\\\\"%Y-%m-%d %H:%M:%S.%3N\\\\\\\")\\\"",
    "$(date +\"%Y-%m-%d %H:%M:%S.%3N\")",
  );

  script.push_str(&format!("echo \"{}\" >> {}\n", escaped, abs_path.display()));
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
  script.push_str("SBM_STATUS = $?\n\n");

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
