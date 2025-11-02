mod configs;
mod includes;
mod jobs;
mod utils;
pub mod variables;

#[cfg(test)]
mod tests;

use thiserror::Error;

pub use jobs::{ParsedJob, parse_jobs_from_file};
pub use configs::parse_clusters_configs_from_file;

#[derive(Error, Debug)]
pub enum ParserError {
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("YAML parsing failed: {0}")]
  YamlParseFailed(#[from] saphyr::ScanError),
  #[error(
    "The file {0} is being included multiple times. Check if it has been included from multiple files or if there is a circular include (ex. FILE 1 -> FILE 2 -> FILE 1)."
  )]
  CircularInclude(String),
  #[error("YAML file is empty!")]
  YamlEmpty,
  #[error("Eval Error: {0}")]
  EvalError(String),
  #[error("Missing Key: {0}")]
  MissingKey(String),
  #[error("Cluster config file is empty!")]
  EmptyClusterConfig,
  #[error("Wrong type for value \"{0}\", expected type {1}")]
  WrongType(String, String),
  #[error("Include error: {0} is neither a string nor a sequence")]
  IncludeWrongType(String),
  #[error("Scheduler \"{0}\" is invalid. Valid options are: Local, Slurm, Pbs")]
  InvalidScheduler(String),
  #[error("Invalid parameter \"{0}\" for scheduler {1:?}")]
  InvalidParameterForScheduler(String, String),
}