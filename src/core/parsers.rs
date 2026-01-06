mod config_parser;
mod includes;
mod job_parser;
mod yaml_parser;
mod combination_generator;
mod variable_parser;
mod multi_hashmap;
mod template_parser;

#[cfg(test)]
mod tests;

use thiserror::Error;

pub use config_parser::parse_clusters_configs_from_file;
pub use job_parser::{ParsedJob, parse_jobs_from_file};

#[derive(Error, Debug)]
pub enum ParserError {
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("YAML parsing failed: {0}")]
  YamlParseFailed(#[from] saphyr::ScanError),
  #[error(
    "The file {0} is being included multiple times. Check if it has been included from multiple files or if there is a circular include (ex. FILE 1 -> FILE 2 -> FILE 1)."
  )]
  MultipleInclude(String),
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
  #[error("Undefined variable: {0}")]
  UndefinedVariable(String),
  #[error("Cyclic dependency detected among variables")]
  CyclicDependency(),
  #[error("Invalid indexed variable: {0}")]
  InvalidIndexedVariable(String),
  #[error("File read error for file {0}: {1}")]
  FileReadError(String, String),
  #[error("Cyclic dependency detected among variables")]
  CyclicVariableDependency(),
}
