use chrono::{DateTime, NaiveTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::create_dir_all;
use std::io::{Error, Write};
use std::path::{Path, PathBuf};

use crate::core::database::models::Status;
use crate::core::{database::models::Job, jobs::JobError};

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
  BashVariable(String), // The string must contain the bash variable name in the format "${VAR}"
}

/// Escapes a string for use within single quotes in a printf command
/// Handles ${...} bash variables and $(...) command substitutions specially to allow expansion
pub fn escape_for_printf(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(ch) = chars.next() {
        match ch {
            // Escape single quotes by ending the single-quoted string,
            // adding an escaped single quote, and starting a new single-quoted string
            '\'' => result.push_str("'\\''"),
            // Escape backslashes for printf
            '\\' => result.push_str("\\\\"),
            // Handle ${...} and $(...) for bash expansion
            '$' => {
                match chars.peek() {
                    Some(&'{') => {
                        // Handle ${VAR} - close quote, add variable unquoted, reopen quote
                        result.push_str("'\"${");
                        chars.next(); // consume '{'
                        
                        // Copy until closing brace
                        while let Some(inner_ch) = chars.next() {
                            if inner_ch == '}' {
                                result.push_str("}\"'");
                                break;
                            }
                            result.push(inner_ch);
                        }
                    }
                    Some(&'(') => {
                        // Handle $(cmd) - close quote, add command substitution unquoted, reopen quote
                        result.push_str("'\"$(");
                        chars.next(); // consume '('
                        
                        let mut depth = 1;
                        // Copy until matching closing paren, handling nested parens
                        while let Some(inner_ch) = chars.next() {
                            match inner_ch {
                                '(' => {
                                    depth += 1;
                                    result.push(inner_ch);
                                }
                                ')' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        result.push_str(")\"'");
                                        break;
                                    }
                                    result.push(inner_ch);
                                }
                                _ => result.push(inner_ch),
                            }
                        }
                    }
                    _ => result.push('$'),
                }
            }
            _ => result.push(ch),
        }
    }
    result
}

pub fn serialize_log_entry(log: JobLog, additional_data: Option<serde_json::Value>) -> Value {
    let mut log_entry = match log {
        JobLog::BashVariable(var_name) => {
            json!({
                "data": { var_name.clone(): format!("${{{}}}", var_name) },
                "type": "BashVariable"
            })
        }
        other => json!(other),
    };

    if let Some(data) = additional_data {
        log_entry
            .as_object_mut()
            .unwrap()
            .insert("additional".to_string(), data);
    }

    // Add timestamp placeholder
    log_entry
        .as_object_mut()
        .unwrap()
        .insert("timestamp".to_string(), Value::String("__TIMESTAMP__".to_string()));

    log_entry
}

/// Parse time string in format "HH:MM:SS" or "D-HH:MM:SS" to seconds
/// Compatible with SLURM, PBS, and local schedulers
pub fn parse_time_to_seconds(time_str: &str) -> Result<u64, JobError> {
  // Split possible "D-" prefix
  let (days, time_part) = if let Some((d, t)) = time_str.split_once('-') {
    let days: u64 = d
      .parse()
      .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;
    (days, t)
  } else {
    (0, time_str)
  };

  // Parse HH:MM:SS using chrono
  let time = NaiveTime::parse_from_str(time_part, "%H:%M:%S")
    .map_err(|_| JobError::InvalidTimeFormat(time_str.to_string()))?;

  // Convert to total seconds
  let total_seconds = days * 86_400 + time.num_seconds_from_midnight() as u64;

  Ok(total_seconds)
}

pub fn get_timestamp_string() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
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
