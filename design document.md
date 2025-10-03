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
    - Management of jobs & filtering
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
- max_jobs

Config:
- id (primary key)
- config_name
- cluster_id (foreign key to Cluster)
- flags (json)
- env (json)

Job
- id (primary key)
- job_name
- config_id (foreign key to Config)
- submit_time
- directory
- command
- status (enum: virtualqueue, queued, running, completed, failed)
- job_id
- start_time
- end_time
- preprocess
- postprocess
- archived
- variables (json)

VirtualQueue
- enqueued_jobs

### Cluster configuration file

variable types:
 - string
 - directory (@dir(...)) (can only be scalar)
 - file (@file(...)) (can only be scalar)
 - array
 - map (key-value pairs, keys are strings, values can be string or array)
preprocess/command/postprocess can be an array or a scalar

substitution syntax:
  - {var}: simple variable substitution
  - python block:
    sbatchman variables are prepended with $, combinations need to be computed to create all jobs
    {{$var}}: access simple variable
    {{$map[key]}}: access map by key
    {{$map[$var]}}: access map by sbatchman variable

include prepends the included file

predefined variables in job config:
  - work_dir: working directory where sbatchman is run
  - out_dir: output directory where job results are stored
  - config_name: name of the cluster config used
  - cluster_name: name of the cluster

```yaml
# variables.yaml
variables:
  interconnect:
    # default: ["cpu", "gpu"]
    cluster_map:
    {
      "clusterA": ["ethernet", "infiniband"],
      "clusterB": ["ethernet"]
    }
  partition:
    cluster_map:
    {
      "clusterA": ["cpu_A", "gpu_A"],
      "clusterB": ["cpu_B"]
    }
  qos: {
    "cpu_A": "normalcpu",
    "gpu_A": "normalgpu",
    "gpu_B": "normalgpu",
  }
  ncpus: [4, 8]
  datasets: @dir(datasets/)  # directory, each file is a value
  scales: @file(scales.txt) # file, each line is a value
```

```yaml
# clusters_configs.yaml
include: variables.yaml

clusters:
  clusterA:
    scheduler: slurm
    default_conf:
      account: "example_default_account"
    configs:
      - name: job_{partition}_{ncpus}
        partition: "{partition}"
        qos: "{{$qos[$partition]}}"
        cpus_per_task: "{ncpus}"
        mem: ["4G", "8G", "16G"]
        time: "01:00:00"
        flags: [
          "-G 10",
        ]
        env:
          - "DATASET={dataset}"
          - "OMP_NUM_THREADS={ncpus}"

  clusterB:
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
include: variables.yaml
python:
  header: "import os\ndef my_func(x):\n  return x * 2"

variables:
  dataset_dir: @dir(datasets/images)
  gpu_list: @file(gpus.txt)
  python_command: python3.10 # This is a simple string
  runs: [100, 200]              # Explicit run counts
  # flags: ['--flag1', '--flag2'] # Default CLI flags
  flags:
    default: [100, 200]
    cluster_map: {
      "clusterA": ['--flag1', '--flag2']
      "clusterB": ['--flag3']
    }

command: python run.py --input {dataset_dir} --runs {runs} --gpus {gpu_list} {flags}
preprocess: echo "Preparing dataset {dataset_dir}"
postprocess: echo "Cleaning up after {dataset_dir}"

variables:
  partition: [cpu, gpu]

jobs:
  - name: baseline_experiment
    cluster_config: gpu_config_{gpu_list}
    scheduler: local
    variants:
      - name: flag_{flags}           # Variant name
      - name: custom_flag
        variables:
          flags: ['--flag3']         # Override default flags

  - name: other_experiment
    cluster_config: "{partition}_config"
    variables:
      runs: [300, 400]               # Override global runs
    command: python custom.py --file {dataset_dir} --runs {runs}
    preprocess: echo "Custom preprocess for config custom_exp_{dataset_dir}"
    # Inherits top-level postprocess
    variants:
      - name: variant1
        variables:
          partition: [cpu]
      - name: variant2
        variables:
          partition: ["cpu", "gpu"]
        command: python custom_1.py --file {dataset_dir} --runs {runs}

  - name: weak_scaling
    cluster_config: other_cluster_config
    variables:
      weak_scaling_params: [(1, 1024), (2, 2048), (4, 4098)]
    command: python custom.py --n_cpus {weak_scaling.1} --array_size {weak_scaling.2}
```