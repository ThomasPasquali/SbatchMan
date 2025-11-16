use std::fs;

use chrono::{Datelike, Timelike};
use serde::Deserialize;
use serde_json::json;
use tempfile::TempDir;

use crate::core::{
  cluster_configs::ClusterConfig,
  database::models::Status,
  jobs::{
    JobLog, SchedulerTrait,
    local::LocalScheduler,
    tests::{create_test_cluster, create_test_config, create_test_config_timeout, create_test_job},
    utils::parse_timestamp,
  },
};

fn as_u64_coerce(v: &serde_json::Value) -> Option<u64> {
  v.as_u64().or_else(|| v.as_str()?.parse().ok())
}

// ============================================================================
// "Manual" tests
// ============================================================================

#[test]
fn test_job_launch() {
  let path = "./test_job";
  let _ = fs::remove_dir_all(path);
  let mut job = create_test_job(1, path);
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let local_scheduler = LocalScheduler::default();

  assert!(
    local_scheduler
      .launch_job(&mut job, &ClusterConfig::new(&cluster, &config))
      .is_ok()
  );
}

#[test]
fn test_job_launch_timeout() {
  let path = "./test_job_timeout";
  let _ = fs::remove_dir_all(path);
  let mut job = create_test_job(1, path);
  job.command = String::from("sleep 2");
  let config = create_test_config_timeout(1, 1);
  let cluster = create_test_cluster(1);

  let local_scheduler = LocalScheduler::default();
  let res = local_scheduler.launch_job(&mut job, &ClusterConfig::new(&cluster, &config));

  assert!(res.is_ok());
  let logs = job.read_log_entries().expect("Could not read logs");
  assert!(
    logs.iter().any(
      |s| serde_json::from_value::<JobLog>(s.clone()).is_ok_and(|log| match log {
        JobLog::StatusUpdate(Status::Timeout) => true,
        _ => false,
      })
    )
  );
}

// ============================================================================
// Tests for LocalScheduler::get_number_of_enqueued_jobs
// ============================================================================

#[test]
fn test_get_number_of_enqueued_jobs_always_zero() {
  let scheduler = LocalScheduler::default();
  let result = scheduler.get_number_of_enqueued_jobs().unwrap();
  assert_eq!(result, 0);
}

// ============================================================================
// Tests for LocalScheduler::launch_job
// ============================================================================

#[test]
fn test_launch_job_creates_script_file() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job_launch");
  let mut job = create_test_job(1, job_dir.to_str().unwrap());
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  let result = scheduler.launch_job(&mut job, &ClusterConfig::new(&cluster, &config));

  assert!(result.is_ok());
  assert!(job_dir.join("job.sh").exists());
}

#[test]
fn test_launch_job_creates_logs() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job_launch_logs");
  let mut job = create_test_job(1, job_dir.to_str().unwrap());
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  scheduler
    .launch_job(&mut job, &ClusterConfig::new(&cluster, &config))
    .unwrap();

  let entries = job.read_log_entries().unwrap();

  // Check for expected log events
  let events: Vec<&str> = entries
    .iter()
    .map(|e| e["type"].as_str().unwrap())
    .collect();

  assert!(events.contains(&"Metadata"));
  assert!(events.contains(&"StatusUpdate"));
  assert_eq!(events.iter().filter(|v| **v == "StatusUpdate").count(), 3);
}

#[test]
fn test_launch_job_creates_stdout_stderr() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job_output");
  let mut job = create_test_job(1, job_dir.to_str().unwrap());
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  scheduler
    .launch_job(&mut job, &ClusterConfig::new(&cluster, &config))
    .unwrap();

  assert!(job_dir.join("stdout.log").exists());
  assert!(job_dir.join("stderr.log").exists());

  let stdout_content = fs::read_to_string(job_dir.join("stdout.log")).unwrap();
  assert!(stdout_content.contains("Hello World"));
}

#[test]
fn test_launch_job_failing_command() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job_fail");
  let mut job = create_test_job(1, job_dir.to_str().unwrap());
  job.command = "$(exit 7)".to_string();
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  let result = scheduler.launch_job(&mut job, &ClusterConfig::new(&cluster, &config));

  // Job should complete but log the failure
  assert!(result.is_ok());

  let entries = job.read_log_entries().expect("Could not read log");

  entries
    .iter()
    .find(|e| e["type"] == "StatusUpdate" && e["data"] == "Failed")
    .expect("Could not find Failed update log");
  let bash_var_update = entries
    .iter()
    .find(|e| e["type"] == "BashVariable" && e["data"].get("SBM_EXIT_CODE").is_some())
    .expect("Could not find exit code log");
  let pid_update = entries
    .iter()
    .find(|e| {
      e["type"] == "Variable"
        && e["data"].is_array()
        && e["data"].as_array().unwrap()[0].as_str() == Some("PID")
    })
    .expect("Could not find exit code log");

  assert!(as_u64_coerce(&pid_update["data"].as_array().unwrap()[1]).unwrap() as i64 > 0);
  assert!(as_u64_coerce(&bash_var_update["data"]["SBM_EXIT_CODE"]).is_some());
  assert_eq!(
    as_u64_coerce(&bash_var_update["data"]["SBM_EXIT_CODE"]).unwrap(),
    7
  );
}

#[test]
fn test_launch_job_with_timeout() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job_timeout");
  let mut job = create_test_job(1, job_dir.to_str().unwrap());
  job.command = "sleep 10".to_string();
  let mut config = create_test_config(1);
  config.flags = json!({"time": "00:00:01"});
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  let result = scheduler.launch_job(&mut job, &ClusterConfig::new(&cluster, &config));

  assert!(result.is_ok());

  let entries = job.read_log_entries().unwrap();

  let timeout_entry = entries
    .iter()
    .find(|e| e["type"] == "StatusUpdate" && e["data"] == "Timeout");
  assert!(timeout_entry.is_some());
}

#[test]
fn test_parse_valid_timestamp() {
  let ts_str = "2025-10-28 09:40:12.366";
  let dt = parse_timestamp(ts_str).expect("Failed to parse timestamp");

  assert_eq!(dt.year(), 2025);
  assert_eq!(dt.month(), 10);
  assert_eq!(dt.day(), 28);
  assert_eq!(dt.hour(), 9);
  assert_eq!(dt.minute(), 40);
  assert_eq!(dt.second(), 12);
  assert_eq!(dt.and_utc().timestamp_subsec_millis(), 366);
}

#[test]
fn test_parse_invalid_timestamp() {
  let bad_ts = "2025/10/28 09:40:12"; // Wrong format
  let result = parse_timestamp(bad_ts);
  assert!(
    result.is_err(),
    "Expected parsing to fail for invalid format"
  );
}

#[test]
fn test_launch_job_logs_duration() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job_duration");
  let mut job = create_test_job(1, job_dir.to_str().unwrap());
  job.command = "sleep 0.1".to_string();
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  scheduler
    .launch_job(&mut job, &ClusterConfig::new(&cluster, &config))
    .unwrap();

  let entries = job.read_log_entries().unwrap();

  let start_entry = entries
    .iter()
    .find(|e| e["type"] == "StatusUpdate" && e["data"] == "Running")
    .unwrap();
  let start = parse_timestamp(start_entry["timestamp"].as_str().unwrap()).unwrap();

  let end_entry = entries
    .iter()
    .find(|e| e["type"] == "StatusUpdate" && e["data"] == "Completed")
    .unwrap();
  let end = parse_timestamp(end_entry["timestamp"].as_str().unwrap()).unwrap();

  let duration_ms = (end - start).num_milliseconds();
  assert!(duration_ms >= 100); // At least 100ms for sleep 0.1
  assert!(duration_ms < 5000); // But not too long
}

#[test]
fn test_launch_job_preprocessor_postprocessor() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job_full_pipeline");
  let mut job = create_test_job(1, job_dir.to_str().unwrap());
  job.preprocess = Some("echo 'PRE' > pre.txt".to_string());
  job.command = "echo 'MAIN' > main.txt".to_string();
  job.postprocess = Some("echo 'POST' > post.txt".to_string());
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  scheduler
    .launch_job(&mut job, &ClusterConfig::new(&cluster, &config))
    .unwrap();

  // Check that all files were created
  assert!(temp_dir.path().join("pre.txt").exists());
  assert!(temp_dir.path().join("main.txt").exists());
  assert!(temp_dir.path().join("post.txt").exists());
}
