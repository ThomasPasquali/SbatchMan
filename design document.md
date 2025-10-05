# Design Document: `sbatchman`

## Overview

### Project Vision

`sbatchman` is tool for managing, submitting, and tracking jobs on High-Performance Computing (HPC) clusters. Job configurations can be defined using simple YAML configuration files and submitted jobs can be monitored and managed using an interactive Terminal User Interface (TUI). `sbatchman` provides also a Python library to collect the outputs of jobs for further analysis and plotting.

### Core Features

  * **Job Configuration:** Define clusters and jobs using a powerful and flexible YAML configuration, with automatic generation of job variants based on parameter combinations.
  * **Interactive TUI:** Monitor active jobs, view logs, manage configurations, and browse job history through an intuitive terminal interface.
  * **Python Library:** Collect job results using a simple Python library, enabling easy integration with data analysis and plotting tools.
  * **Reliability:** All state is stored in a local SQLite database.
  * **Portability:** Export and import jobs, including their results, as self-contained `.zip` archives for easy sharing and reproducibility.

### Architecture

The system is composed of three primary components:

  * **Rust Core:** An efficient engine responsible for all core logic, including configuration parsing, job scheduling and state management.
  * **TUI Frontend:** An interactive terminal application for real-time job monitoring and management.
  * **Python Library:** A Python wrapper around the Rust core, enabling scripting and integration with other Python libraries for data analysis and plotting.

## Technical Specification

### Core Operations (Rust API)

The Rust core exposes the following primary functions for managing jobs, configurations, and the application state.

| Function                                     | Description                                                               | Return Type      |
| -------------------------------------------- | ------------------------------------------------------------------------- | ---------------- |
| `run_jobs_from_file(path: &str)`             | Parses and launches all jobs defined in a job configuration file.         | `Result<()>`      |
| `parse_jobs_from_file(path: &str)`           | Parses a job configuration file and returns the generated jobs without launching them. | `Result<Vec<Job>>` |
| `launch_jobs(jobs: &[Job])`                   | Submits a slice of `Job` objects to the appropriate cluster schedulers.   | `Result<()>`      |
| `get_jobs(filter: JobFilter)`                | Retrieves a list of jobs from the database that match the filter criteria. | `Result<Vec<Job>>` |
| `export_jobs(filter: JobFilter, path: &str)` | Exports jobs matching the filter (including results) to a `.zip` file.    | `Result<()>`      |

  * `import_jobs(path: &str)`                  | Imports jobs from a `.zip` archive into the database.                     | `Result<()>`      |
    | `import_cluster_configs_from_file(path: &str)` | Imports cluster configurations from a YAML file into the database.        | `Result<()>`      |
    | `get_cluster_config(name: &str)`             | Retrieves a specific cluster configuration by name.                       | `Result<ClusterConfig>` |
    | `get_sbatchman_path()`                       | Returns the root directory path for `sbatchman` data and databases.       | `Result<String>`  |
    | `init_db(path: &str)`                        | Initializes the SQLite database at the given path.                        | `Result<()>`      |
    | `migrate_db(path: &str)`                     | Applies necessary schema migrations to the database.                      | `Result<()>`      |

### TUI Frontend

The TUI provides the primary interactive experience for the user.

  * **Technology Stack:**
    * **CLI Parsing:** `Clap`
    * **TUI Rendering:** `Ratatui`
    * *(Inspiration: [MAIF/yozefu](https://github.com/MAIF/yozefu))*
  * **Features:**
    * **Job Monitoring:** View lists of active, queued, and archived jobs with real-time updates and pagination support.
    * **Log Viewer:** Directly view the `stdout`/`stderr` logs for any selected job.
    * **Job Management:** Perform operations on jobs (e.g., cancel, archive, re-run).
    * **Advanced Filtering:** Apply filters to narrow down the list of displayed jobs.
    * **Configuration Management:** View and manage cluster configurations stored in the database.

### 2.3. Python Library

A Python library will be provided to access the core `sbatchman` functionality.

  * **Binding Technology:** `PyO3`
  * **Purpose:** To enable users to retrieve jobs directly from within Python scripts, Jupyter notebooks, or other analysis tools.

## Storage and Data Model

`sbatchman` uses a local SQLite database to persist all state.

### Database Schema

#### **Table: `Cluster`**

Stores information about available compute clusters.

  * `id` (INTEGER, Primary Key)
  * `cluster_name` (TEXT)
  * `scheduler` (TEXT, e.g., "slurm", "pbs")
  * `max_jobs` (INTEGER)

#### **Table: `Config`**

Stores specific configurations for submitting jobs to a cluster.

  * `id` (INTEGER, Primary Key)
  * `config_name` (TEXT)
  * `cluster_id` (INTEGER, Foreign Key to `Cluster.id`)
  * `flags` (TEXT, JSON Array)
  * `env` (TEXT, JSON Array)

#### **Table: `Job`**

Stores detailed information for every job generated and submitted.

  * `id` (INTEGER, Primary Key)
  * `job_name` (TEXT)
  * `config_id` (INTEGER, Foreign Key to `Config.id`)
  * `submit_time` (DATETIME)
  * `directory` (TEXT)
  * `command` (TEXT)
  * `status` (TEXT, Enum: `virtualqueue`, `queued`, `running`, `completed`, `failed`)
  * `job_id` (INTEGER)
  * `start_time` (DATETIME)
  * `end_time` (DATETIME)
  * `preprocess` (TEXT)
  * `postprocess` (TEXT)
  * `archived` (BOOLEAN)
  * `variables` (TEXT, JSON Object)

#### **Table: `VirtualQueue`**

Manages jobs that are pending submission to the cluster scheduler due to limits.

  * `enqueued_jobs` (TEXT, JSON array of Job IDs)

### Job Filtering

Jobs can be queried from the database using a flexible filter specification.

#### Rust `JobFilter` Struct

```rust
pub struct JobFilter {
  pub name: Option<String>,
  pub status: Option<JobStatus>,
  pub cluster: Option<String>,
  pub config: Option<String>,
  pub archived: Option<bool>,
  pub submit_time_range: Option<(NaiveDateTime, NaiveDateTime)>,
  pub end_time_range: Option<(NaiveDateTime, NaiveDateTime)>,
}
```

#### Supported Filter Criteria

  * **Name:** Partial, case-insensitive match on the job name.
  * **Status:** Exact match on job status (`pending`, `running`, etc.).
  * **Cluster:** Filter by the name of the cluster the job is associated with.
  * **Configuration:** Filter by the name of the configuration used for the job.
  * **Archived:** Filter jobs based on their archived status.
  * **Submit/End Time Range:** Filter jobs that were submitted or ended within a specific date-time window.

## Configuration File Specification

`sbatchman` uses YAML for defining clusters and jobs. This system is designed to be highly flexible, enabling the generation of many job variants from a concise definition.

### Syntax and Concepts

  * **Includes:** Configuration files can be composed together using the `include: <path>` directive.
  * **Variable Types:**
    * **string:** A standard string value.
    * **array:** A list of values, used for generating combinatorial job variants.
    * **map:** A key-value dictionary.
    * **per_cluster:** A special mapping that allows different values for different clusters.
    * **`@dir path`:** A special directive that expands to an array of file/directory names within the specified path.
    * **`@file path`:** A special directive that expands to an array of lines from the specified file.
  * **Substitution Syntax:**
    * **Simple Substitution:** `{var}` is replaced by the value of the variable `var`. For maps, the syntax `{map[key]}` is used. If `key` is a variable, it should be prefixed with `$`, e.g., `{map[$var]}`.
    * **Python Block:** `{{ ... }}` allows for inline Python expressions for generating variables dynamically.
      * `sbatchman` variables are accessible within the block and must be prefixed with a `$`.
  * **Predefined Variables:**
    * `work_dir`: The working directory where `sbatchman` was invoked.
    * `out_dir`: The output directory for the job's results.
    * `config_name`: The name of the cluster configuration being used.
    * `cluster_name`: The name of the cluster being used.

### 4.2. Example: Cluster Configuration (`clusters_configs.yaml`)

```yaml

# variables.yaml

variables:
  interconnect:
    # default: ["cpu", "gpu"]
    per_cluster:
      clusterA: ["ethernet", "infiniband"]
      clusterB: ["ethernet"]

  partition:
    per_cluster:
      clusterA: ["cpu_A", "gpu_A"],
      clusterB: ["cpu_B"]

  qos:
    map:
      "cpu_A": "normalcpu"
      "gpu_A": "normalgpu"
      "gpu_B": "normalgpu"

  ncpus: [4, 8]

  datasets: @dir datasets/ # directory, each file is a value
  scales: @file scales.txt # file, each line is a value

```

```yaml
# clusters_configs.yaml
include: variables.yaml

clusters:
  clusterA:
    scheduler: slurm
    defaults:
      account: "example_default_account"
    configs:
      - name: job_{partition}_{ncpus}
        params:
          partition: "{partition}"
          qos: "{qos[$partition]}"
          cpus_per_task: "{ncpus}"
          mem: ["4G", "8G", "16G"]
          time: "01:00:00"
        options:
          - "-G 10"
        env:
          - "DATASET={dataset}"
          - "OMP_NUM_THREADS={ncpus}"

  clusterB:
    scheduler: pbs
    configs:
      - name: "mem_job_{mem}"
        options:
          - "--cpus: 2"
          - "--mem: {mem}"
          - "--walltime: 01:00:00"
```

### 4.3. Example: Job Configuration (`jobs.yaml`)

```yaml
# jobs.yaml
include: variables.yaml
python:
  header: "import os\ndef my_func(x):\n  return x * 2"

variables:
  dataset_dir: @dir datasets/images
  gpu_list: @file gpus.txt
  python_command: python3.10
  runs: [100, 200]
  flags:
    default: ['--flag_default']
    per_cluster:
      "clusterA": ['--flag1', '--flag2'],
      "clusterB": ['--flag3']

command: python run.py --input {dataset_dir} --runs {runs} --gpus {gpu_list} {flags}
preprocess: echo "Preparing dataset {dataset_dir}"
postprocess: echo "Cleaning up after {dataset_dir}"

jobs:
  - name: baseline_experiment
    cluster_config: gpu_config_{gpu_list}
    scheduler: local
    variants:
      - name: flag_{flags}
      - name: custom_flag
        variables:
          flags: ['--flag3']

  - name: other_experiment
    cluster_config: "{partition}_config"
    variables:
      runs: [300, 400]
      partition: [cpu, gpu]
    command: python custom.py --file {dataset_dir} --runs {runs}
    preprocess: echo "Custom preprocess for config custom_exp_{dataset_dir}"

  - name: weak_scaling
    cluster_config: other_cluster_config
    variables:
      weak_scaling_params: [(1, 1024), (2, 2048), (4, 4098)]
    command: python custom.py --n_cpus {weak_scaling.1} --array_size {weak_scaling.2}
```

## Implementation Plan

The development will be broken down into the following primary tasks:

1.  **Core Operations:** Implement the fundamental Rust logic for parsing, state management, and job submission.
2.  **Storage:** Set up the SQLite database, schema, and migration system.
3.  **TUI:** Develop the interactive terminal frontend using Clap and Ratatui.
4.  **Python Library:** Create the PyO3 bindings to expose the core API to Python.
5.  **Testing:** Implement a comprehensive suite of unit and integration tests for all components.