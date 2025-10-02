### Main features
- Import cluster config from YAML file
- Run jobs from job config YAML file
- List queued and finished jobs
- Export/import jobs (including results) as .zip file
- Python library for job management

### Frontend
- CLI with TUI
  - Implemented using Clap (for parsing CLI arguments) and Ratatui (for TUI)
  (inspiration: https://github.com/MAIF/yozefu)
  - Features:
    - Monitor active and archived jobs
      - View job logs
    - View and manage cluster configs

### Core operations (Rust)
Main functions:
- Run jobs from job config file: `run_jobs_from_file(path: &str) -> Result<()>`
  - Parse jobs from job config file: `parse_jobs_from_file(path: &str) -> Result<Vec<Job>>`
  - Launch jobs: `launch_jobs(jobs: &[Job]) -> Result<()>`
- Get jobs: `get_jobs(filter: JobFilter) -> Result<Vec<Job>>`
- Export jobs: `export_jobs(filter: JobFilter, path: &str) -> Result<()>`
- Import jobs: `import_jobs(path: &str) -> Result<()>`
- Import cluster configs from file: `import_cluster_configs_from_file(path: &str) -> Result<()>`
- Get cluster config from name: `get_cluster_config(name: &str) -> Result<ClusterConfig>`
- Get sbatchman path: `get_sbatchman_path() -> Result<String>`
- Initialize database: `init_db(path: &str) -> Result<()>`
- Migrate database schema: `migrate_db(path: &str) -> Result<()>`

### Job Filter Specification

The job filter allows users to query jobs based on specific criteria. It is implemented as a struct in Rust and supports filtering by multiple fields.

#### Rust Struct
```rust
pub struct JobFilter {
  pub name: Option<String>,          // Filter by job name (partial match)
  pub status: Option<JobStatus>,     // Filter by job status (enum: pending, queued, running, completed, failed)
  pub cluster: Option<String>,       // Filter by cluster name
  pub config: Option<String>,        // Filter by configuration name
  pub archived: Option<bool>,        // Filter by archived status
  pub submit_time_range: Option<(NaiveDateTime, NaiveDateTime)>, // Filter by submit time range
  pub end_time_range: Option<(NaiveDateTime, NaiveDateTime)>,    // Filter by end time range
}
```

#### Supported Filters
- **Name**: Partial match on the job name.
- **Status**: Filter by job status (e.g., `running`, `completed`).
- **Cluster**: Filter jobs associated with a specific cluster.
- **Configuration**: Filter jobs associated with a specific configuration.
- **Archived**: Filter jobs based on whether they are archived or not.
- **Submit Time Range**: Filter jobs submitted within a specific time range.
- **End Time Range**: Filter jobs that ended within a specific time range.

#### Example Usage
```rust
let filter = JobFilter {
  name: Some("experiment".to_string()),
  status: Some(JobStatus::Running),
  cluster: None,
  config: None,
  archived: Some(false),
  submit_time_range: None,
  end_time_range: None,
};

let jobs = get_jobs(filter).unwrap();
```

### Storage description
SQLite database with the following tables:
Cluster:
- id (primary key)
- cluster_name
- scheduler

Config:
- id (primary key)
- config_name
- cluster_id (foreign key to Cluster)
- flags (json)
- env (json)
- max_jobs

Job
- id (primary key)
- job_name
- config_id (foreign key to Config)
- submit_time
- directory
- command
- status (enum: pending, queued, running, completed, failed)
- job_id
- end_time
- preprocess
- postprocess
- archived
- variables (json)

### Cluster configuration file
```yaml
defaults:
  variables:
    partition: [cpu, gpu]
    ncpus: [4, 8]
    dataset: datasets/   # directory, each file is a value
    mem: ["8gb", "16gb"]

clusters:
  - name: clusterA
    scheduler: slurm
    default_conf:
      account: "example_default_account"
    configs:
      - name: job_{partition}_{ncpus}\
        flags: [
          "--partition {partition}",
          "--cpus-per-task={ncpus}",
          "--mem={mem}",
          "--time=01:00:00",
        ]
        env:
          - "DATASET={dataset}"
          - "OMP_NUM_THREADS={ncpus}"

  - name: clusterB
    scheduler: pbs
    configs:
      - name: "mem_job_{mem}"
        flags: [
          "--cpus: 2",
          "--mem: {mem}",
          "--walltime: 01:00:00",
        ]
```

### Job configuration file
```yaml
defaults:
  variables:
    dataset_dir: @dir(datasets/images)
    gpu_list: @file(gpus.txt)
    python_command: python3.10 # This is a simple string
    runs: [100, 200]              # Explicit run counts
    flags: ['--flag1', '--flag2'] # Default CLI flags
  command: python run.py --input {dataset_dir} --runs {runs} --gpus {gpu_list} {flags}
  preprocess: echo "Preparing dataset {dataset_dir}"
  postprocess: echo "Cleaning up after {dataset_dir}"

jobs:
  - name: baseline_experiment
    cluster_config: gpu_config_{gpu_list}
    variants:
      - name: flag_{flags}           # Variant name
      - name: custom_flag
        variables:
          flags: ['--flag3']         # Override default flags

  - name: other_experiment
    cluster_config: other_cluster_config
    cluster_allowlist: [clusterA, clusterB] # Restrict these jobs to clusters A and B
    variables:
      runs: [300, 400]               # Override global runs
    command: python custom.py --file {dataset_dir} --runs {runs}
    preprocess: echo "Custom preprocess for config custom_exp_{dataset_dir}"
    # Inherits top-level postprocess
    variants:
      - name: custom_program
        variables:
          dataset_dir:
            path: datasets/test/
      - name: custom_program1
        variables:
          dataset_dir:
            path: datasets/test1/
        command: python custom_1.py --file {dataset_dir} --runs {runs}

  - name: weak_scaling
    cluster_config: other_cluster_config
    variables:
      weak_scaling_params: [(1, 1024), (2, 2048), (4, 4098)]
    command: python custom.py --n_cpus {weak_scaling.1} --array_size {weak_scaling.2}
```