use crate::core::cluster_configs::ClusterConfig;
use crate::core::database::models::{Cluster, Config, Job, Scheduler, Status};
use crate::core::jobs::local::LocalScheduler;
use crate::core::jobs::utils::parse_time_to_seconds;
use crate::core::jobs::{JobError, SchedulerTrait};

use log::debug;
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

mod local;

fn create_test_job(id: i32, directory: &str) -> Job {
  Job {
    id,
    job_name: format!("test_job_{}", id),
    config_id: 1,
    submit_time: Some(1000),
    directory: directory.to_string(),
    command: "echo 'Hello World'".to_string(),
    status: Status::Queued,
    job_id: None,
    end_time: None,
    preprocess: None,
    postprocess: None,
    archived: None,
    variables: json!({}),
  }
}

fn create_test_config(id: i32) -> Config {
  Config {
    id,
    config_name: "test_config".to_string(),
    cluster_id: 1,
    flags: json!({}),
    env: json!({}),
  }
}

fn create_test_cluster(id: i32) -> Cluster {
  Cluster {
    id,
    cluster_name: "test_cluster".to_string(),
    scheduler: Scheduler::Local,
    max_jobs: Some(10),
  }
}

fn read_log_entries(log_path: &Path) -> Result<Vec<serde_json::Value>, std::io::Error> {
  let content = fs::read_to_string(log_path)?;
  let entries: Vec<serde_json::Value> = content
    .lines()
    .filter(|line| !line.is_empty())
    .map(|line| serde_json::from_str(line).unwrap())
    .collect();
  Ok(entries)
}

// ============================================================================
// Tests for parse_time_to_seconds
// ============================================================================

#[test]
fn test_parse_time_hhmmss_format() {
  assert_eq!(parse_time_to_seconds("01:30:45").unwrap(), 5445);
  assert_eq!(parse_time_to_seconds("00:00:01").unwrap(), 1);
  assert_eq!(parse_time_to_seconds("23:59:59").unwrap(), 86399);
  assert_eq!(parse_time_to_seconds("10:00:00").unwrap(), 36000);
}

#[test]
fn test_parse_time_d_hhmmss_format() {
  assert_eq!(parse_time_to_seconds("1-00:00:00").unwrap(), 86400);
  assert_eq!(parse_time_to_seconds("2-12:30:45").unwrap(), 217845);
  assert_eq!(parse_time_to_seconds("0-01:00:00").unwrap(), 3600);
  assert_eq!(parse_time_to_seconds("7-00:00:00").unwrap(), 604800);
}

#[test]
fn test_parse_time_invalid_formats() {
  assert!(parse_time_to_seconds("invalid").is_err());
  assert!(parse_time_to_seconds("1:2").is_err());
  assert!(parse_time_to_seconds("1:2:3:4").is_err());
  assert!(parse_time_to_seconds("1-2").is_err());
  assert!(parse_time_to_seconds("1-2:3").is_err());
  assert!(parse_time_to_seconds("a:b:c").is_err());
  assert!(parse_time_to_seconds("1-a:b:c").is_err());
}

#[test]
fn test_parse_time_edge_cases() {
  assert_eq!(parse_time_to_seconds("00:00:00").unwrap(), 0);
  assert_eq!(parse_time_to_seconds("0-00:00:00").unwrap(), 0);
}

// ============================================================================
// Tests for prepare_job_directory
// ============================================================================

#[test]
fn test_prepare_job_directory_creates_directory() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("test_job");
  let job = create_test_job(1, job_dir.to_str().unwrap());

  assert!(!job_dir.exists());

  let result = job.prepare_job_directory();
  assert!(result.is_ok());
  assert!(job_dir.exists());

  assert_eq!(job.get_script_path(), job_dir.join("job.sh"));
  assert_eq!(job.get_log_path(), job_dir.join("job.log"));
  assert_eq!(job.get_stdout_path(), job_dir.join("stdout.log"));
  assert_eq!(job.get_stderr_path(), job_dir.join("stderr.log"));
}

#[test]
fn test_prepare_job_directory_existing_directory() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("existing_job");
  let job = create_test_job(1, job_dir.to_str().unwrap());

  fs::create_dir_all(&job_dir).unwrap();
  assert!(job_dir.exists());

  let result = job.prepare_job_directory();
  assert!(result.is_ok());
  assert!(job_dir.exists());
}

#[test]
fn test_prepare_job_directory_nested_paths() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("level1/level2/level3/job");
  let job = create_test_job(1, job_dir.to_str().unwrap());

  let result = job.prepare_job_directory();
  assert!(result.is_ok());
  assert!(job_dir.exists());
}

// ============================================================================
// Tests for add_environment_variables
// ============================================================================

#[test]
fn test_add_environment_variables_empty() {
  let mut script = String::new();
  let cluster = create_test_cluster(1);
  let config = create_test_config(1);
  let cluster_config = ClusterConfig::new(&cluster, &config);

  cluster_config.add_environment_variables(&mut script);
  assert!(script.is_empty());
}

#[test]
fn test_add_environment_variables_single() {
  let mut script = String::new();
  let cluster = create_test_cluster(1);
  let mut config = create_test_config(1);
  config.env = json!({"VAR1": "value1"});
  let cluster_config = ClusterConfig::new(&cluster, &config);

  cluster_config.add_environment_variables(&mut script);
  assert!(script.contains("export VAR1=\"value1\""));
}

#[test]
fn test_add_environment_variables_multiple() {
  let mut script = String::new();
  let cluster = create_test_cluster(1);
  let mut config = create_test_config(1);
  config.env = json!({
      "PATH": "/usr/bin:/bin",
      "PYTHONPATH": "/opt/python",
      "CUDA_VISIBLE_DEVICES": "0,1"
  });
  let cluster_config = ClusterConfig::new(&cluster, &config);

  cluster_config.add_environment_variables(&mut script);
  assert!(script.contains("export PATH=\"/usr/bin:/bin\""));
  assert!(script.contains("export PYTHONPATH=\"/opt/python\""));
  assert!(script.contains("export CUDA_VISIBLE_DEVICES=\"0,1\""));
}

#[test]
fn test_add_environment_variables_non_string_values_included() {
  let mut script = String::new();
  let cluster = create_test_cluster(1);
  let mut config = create_test_config(1);
  config.env = json!({
      "STRING_VAR": "value",
      "NUMBER_VAR": 42,
      "BOOL_VAR": true
  });
  let cluster_config = ClusterConfig::new(&cluster, &config);

  cluster_config.add_environment_variables(&mut script);
  debug!("{}", script);

  assert!(script.contains("export STRING_VAR=\"value\""));
  assert!(script.contains("export NUMBER_VAR=42"));
  assert!(script.contains("export BOOL_VAR=true"));
}

// ============================================================================
// Tests for add_job_commands
// ============================================================================

#[test]
fn test_add_job_commands_only_main() {
  let temp_dir = TempDir::new().unwrap();
  let mut script = String::new();
  let job = create_test_job(1, temp_dir.path().to_str().unwrap());

  job.add_job_commands(&mut script);

  assert!(script.contains("# Main command"));
  assert!(script.contains("echo 'Hello World'"));
  assert!(!script.contains("# Preprocessing"));
  assert!(!script.contains("# Postprocessing"));
}

#[test]
fn test_add_job_commands_with_preprocessing() {
  let temp_dir = TempDir::new().unwrap();
  let mut script = String::new();
  let mut job = create_test_job(1, temp_dir.path().to_str().unwrap());
  job.preprocess = Some("echo 'Starting preprocessing'".to_string());

  job.add_job_commands(&mut script);

  assert!(script.contains("# Preprocessing"));
  assert!(script.contains("echo 'Starting preprocessing'"));
  assert!(script.contains("# Main command"));
}

#[test]
fn test_add_job_commands_with_postprocessing() {
  let temp_dir = TempDir::new().unwrap();
  let mut script = String::new();
  let mut job = create_test_job(1, temp_dir.path().to_str().unwrap());
  job.postprocess = Some("echo 'Cleanup complete'".to_string());

  job.add_job_commands(&mut script);

  assert!(script.contains("# Main command"));
  assert!(script.contains("# Postprocessing"));
  assert!(script.contains("echo 'Cleanup complete'"));
}

#[test]
fn test_add_job_commands_full_pipeline() {
  let temp_dir = TempDir::new().unwrap();
  let mut script = String::new();
  let mut job = create_test_job(1, temp_dir.path().to_str().unwrap());
  job.preprocess = Some("echo 'Pre'".to_string());
  job.postprocess = Some("echo 'Post'".to_string());

  job.add_job_commands(&mut script);

  // Check order
  let pre_pos = script.find("echo 'Pre'").unwrap();
  let main_pos = script.find("echo 'Hello World'").unwrap();
  let post_pos = script.find("echo 'Post'").unwrap();

  assert!(pre_pos < main_pos);
  assert!(main_pos < post_pos);
}

#[test]
fn test_add_job_commands_empty_strings_ignored() {
  let temp_dir = TempDir::new().unwrap();
  let mut script = String::new();
  let mut job = create_test_job(1, temp_dir.path().to_str().unwrap());
  job.preprocess = Some("".to_string());
  job.postprocess = Some("".to_string());

  job.add_job_commands(&mut script);

  assert!(!script.contains("# Preprocessing"));
  assert!(!script.contains("# Postprocessing"));
  assert!(script.contains("# Main command"));
}

// ============================================================================
// Tests for write_log_entry
// ============================================================================

// #[test]
// fn test_write_log_entry_creates_file() {
//   let temp_dir = TempDir::new().unwrap();
//   let log_path = temp_dir.path().join("test.log");
//   let job = create_test_job(1, temp_dir.path().to_str().unwrap());
//   let config = create_test_config(1);
//   let cluster = create_test_cluster(1);

//   assert!(!log_path.exists());

//   let result = write_log_entry(&log_path, "test_event", &job, &config, &cluster, None);
//   assert!(result.is_ok());
//   assert!(log_path.exists());
// }

// #[test]
// fn test_write_log_entry_contains_required_fields() {
//   let temp_dir = TempDir::new().unwrap();
//   let log_path = temp_dir.path().join("test.log");
//   let job = create_test_job(1, temp_dir.path().to_str().unwrap());
//   let config = create_test_config(1);
//   let cluster = create_test_cluster(1);

//   write_log_entry(&log_path, "test_event", &job, &config, &cluster, None).unwrap();

//   let entries = read_log_entries(&log_path).unwrap();
//   assert_eq!(entries.len(), 1);

//   let entry = &entries[0];
//   assert!(entry.get("timestamp").is_some());
//   assert!(entry.get("timestamp_ms").is_some());
//   assert_eq!(entry["event"], "test_event");
//   assert!(entry.get("job").is_some());
//   assert!(entry.get("config").is_some());
//   assert!(entry.get("cluster").is_some());
// }

// #[test]
// fn test_write_log_entry_with_additional_data() {
//   let temp_dir = TempDir::new().unwrap();
//   let log_path = temp_dir.path().join("test.log");
//   let job = create_test_job(1, temp_dir.path().to_str().unwrap());
//   let config = create_test_config(1);
//   let cluster = create_test_cluster(1);

//   let additional = json!({"pid": 12345, "custom_field": "value"});
//   write_log_entry(&log_path, "test", &job, &config, &cluster, Some(additional)).unwrap();

//   let entries = read_log_entries(&log_path).unwrap();
//   let entry = &entries[0];

//   assert_eq!(entry["additional"]["pid"], 12345);
//   assert_eq!(entry["additional"]["custom_field"], "value");
// }

// #[test]
// fn test_write_log_entry_multiple_entries() {
//   let temp_dir = TempDir::new().unwrap();
//   let log_path = temp_dir.path().join("test.log");
//   let job = create_test_job(1, temp_dir.path().to_str().unwrap());
//   let config = create_test_config(1);
//   let cluster = create_test_cluster(1);

//   write_log_entry(&log_path, "event1", &job, &config, &cluster, None).unwrap();
//   write_log_entry(&log_path, "event2", &job, &config, &cluster, None).unwrap();
//   write_log_entry(&log_path, "event3", &job, &config, &cluster, None).unwrap();

//   let entries = read_log_entries(&log_path).unwrap();
//   assert_eq!(entries.len(), 3);
//   assert_eq!(entries[0]["event"], "event1");
//   assert_eq!(entries[1]["event"], "event2");
//   assert_eq!(entries[2]["event"], "event3");
// }

// #[test]
// fn test_write_log_entry_preserves_job_metadata() {
//   let temp_dir = TempDir::new().unwrap();
//   let log_path = temp_dir.path().join("test.log");
//   let mut job = create_test_job(42, temp_dir.path().to_str().unwrap());
//   job.preprocess = Some("pre.sh".to_string());
//   job.postprocess = Some("post.sh".to_string());
//   job.variables = json!({"key": "value"});
//   let config = create_test_config(1);
//   let cluster = create_test_cluster(1);

//   write_log_entry(&log_path, "test", &job, &config, &cluster, None).unwrap();

//   let entries = read_log_entries(&log_path).unwrap();
//   let entry = &entries[0];

//   assert_eq!(entry["job"]["id"], 42);
//   assert_eq!(entry["job"]["job_name"], "test_job_42");
//   assert_eq!(entry["job"]["command"], "echo 'Hello World'");
//   assert_eq!(entry["job"]["preprocess"], "pre.sh");
//   assert_eq!(entry["job"]["postprocess"], "post.sh");
//   assert_eq!(entry["job"]["variables"]["key"], "value");
// }

// ============================================================================
// Tests for LocalScheduler::create_job_script
// ============================================================================

#[test]
fn test_create_job_script_basic() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job1");
  let job = create_test_job(1, job_dir.to_str().unwrap());
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf().join("work_dir"),
  };
  let script = scheduler
    .create_job_script(&job, &ClusterConfig::new(&cluster, &config))
    .unwrap();

  assert!(script.starts_with("#!/bin/bash"));
  assert!(script.contains("# --- Metadata ---"));
  assert!(script.contains("# Set Working Directory\ncd \""));
  assert!(script.contains("/work_dir\""));
  assert!(script.contains("echo 'Hello World'"));
  assert!(script.contains("SBM_EXIT_CODE=$?"));
}

#[test]
fn test_create_job_script_with_environment() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("job2");
  let job = create_test_job(2, job_dir.to_str().unwrap());
  let mut config = create_test_config(1);
  config.env = json!({"TEST_VAR": "test_value"});
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  let script = scheduler
    .create_job_script(&job, &ClusterConfig::new(&cluster, &config))
    .unwrap();

  assert!(script.contains("export TEST_VAR=\"test_value\""));
}

// #[test]
// fn test_create_job_script_logs_creation() {
//   let temp_dir = TempDir::new().unwrap();
//   let job_dir = temp_dir.path().join("job3");
//   let job = create_test_job(3, job_dir.to_str().unwrap());
//   let config = create_test_config(1);
//   let cluster = create_test_cluster(1);

//   let scheduler = LocalScheduler { launch_base_path: temp_dir.path().to_path_buf() };
//   scheduler
//     .create_job_script(&job, &ClusterConfig::new(&cluster,&config))
//     .unwrap();

//   let log_path = job_dir.join("job.log");
//   assert!(log_path.exists());

//   let entries = read_log_entries(&log_path).unwrap();
//   assert_eq!(entries.len(), 2);
//   assert_eq!(entries[0]["event"], "job_script_creation_start");
//   assert_eq!(entries[1]["event"], "job_script_created");
//   assert!(entries[1]["additional"]["script_length"].as_u64().unwrap() > 0);
// }

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

  let log_path = job_dir.join("job.log");
  assert!(log_path.exists());

  let entries = read_log_entries(&log_path).unwrap();

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
  job.command = "exit 7".to_string();
  let config = create_test_config(1);
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  let result = scheduler.launch_job(&mut job, &ClusterConfig::new(&cluster, &config));

  // Job should complete but log the failure
  assert!(result.is_ok());

  let script_path = job_dir.join("job.sh");
  let content = fs::read_to_string(script_path).unwrap();
  println!("{}", content);
  println!("\n----------stdout---\n{}", job.get_stdout().unwrap());

  let log_path = job_dir.join("job.log");
  let entries = read_log_entries(&log_path).unwrap();

  println!("{:#?}", entries);

  let end_entry = entries
    .iter()
    .find(|e| e["type"] == "StatusUpdate" && e["data"] == "Failed")
    .unwrap();

  assert!(
    end_entry["additional"]["pid"]
      .to_string()
      .parse::<i32>()
      .is_ok()
  );
  assert!(
    end_entry["additional"]["exit_code"]
      .to_string()
      .parse::<i32>()
      .is_ok()
  );
  assert_eq!(
    end_entry["additional"]["exit_code"]
      .to_string()
      .parse::<i32>()
      .unwrap(),
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
  config.flags = json!({"time": "00:00:02"});
  let cluster = create_test_cluster(1);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  let result = scheduler.launch_job(&mut job, &ClusterConfig::new(&cluster, &config));

  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), JobError::Timeout(_)));

  let log_path = job_dir.join("job.log");
  let entries = read_log_entries(&log_path).unwrap();

  let timeout_entry = entries.iter().find(|e| e["event"] == "job_timeout");
  assert!(timeout_entry.is_some());
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

  let log_path = job_dir.join("job.log");
  let entries = read_log_entries(&log_path).unwrap();

  let end_entry = entries
    .iter()
    .find(|e| e["event"] == "job_execution_end")
    .unwrap();

  let duration_ms = end_entry["additional"]["duration_ms"].as_i64().unwrap();
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

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_log_allows_database_reconstruction() {
  let temp_dir = TempDir::new().unwrap();
  let job_dir = temp_dir.path().join("reconstruction_test");
  let mut job = create_test_job(99, job_dir.to_str().unwrap());
  job.variables = json!({"experiment": "test", "seed": 42});
  let mut config = create_test_config(5);
  config.flags = json!({"gpu": true});
  config.env = json!({"PYTHONPATH": "/opt/python"});
  let mut cluster = create_test_cluster(3);
  cluster.max_jobs = Some(50);

  let scheduler = LocalScheduler {
    launch_base_path: temp_dir.path().to_path_buf(),
  };
  scheduler
    .launch_job(&mut job, &ClusterConfig::new(&cluster, &config))
    .unwrap();

  let log_path = job_dir.join("job.log");
  let entries = read_log_entries(&log_path).unwrap();

  // Verify we can reconstruct all entities from any log entry
  let first_entry = &entries[0];

  // Reconstruct Job
  assert_eq!(first_entry["job"]["id"], 99);
  assert_eq!(first_entry["job"]["job_name"], "test_job_99");
  assert_eq!(first_entry["job"]["command"], "echo 'Hello World'");
  assert_eq!(first_entry["job"]["variables"]["experiment"], "test");
  assert_eq!(first_entry["job"]["variables"]["seed"], 42);

  // Reconstruct Config
  assert_eq!(first_entry["config"]["id"], 5);
  assert_eq!(first_entry["config"]["config_name"], "test_config");
  assert_eq!(first_entry["config"]["flags"]["gpu"], true);
  assert_eq!(first_entry["config"]["env"]["PYTHONPATH"], "/opt/python");

  // Reconstruct Cluster
  assert_eq!(first_entry["cluster"]["id"], 3);
  assert_eq!(first_entry["cluster"]["cluster_name"], "test_cluster");
  assert_eq!(first_entry["cluster"]["max_jobs"], 50);
}

// TODO add more
