use std::path::PathBuf;

use crate::core::database::Database;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
pub mod tests;

#[derive(Error, Debug)]
pub enum SbatchmanConfigError {
  #[error("Filesystem error: {0}")]
  FilesystemError(#[from] std::io::Error),
  #[error("Could not find .sbatchman directory")]
  SbatchmanDirNotFound,
  #[error("IO Error: {0}")]
  ConfyError(#[from] confy::ConfyError),
  #[error("Sbatchman config not found")]
  SbatchmanConfigNotFound,
  #[error("Database error: {0}")]
  DatabaseError(#[from] crate::core::database::StorageError),
}

#[derive(Serialize, Deserialize, Default)]
pub struct SbatchmanConfig {
  pub cluster_name: Option<String>,
}

/// Initializes the .sbatchman directory at the specified path:
/// - Creates the .sbatchman directory
/// - Initializes the sbatchman.conf configuration file
/// - Creates the database
pub fn init_sbatchman_dir(path: &PathBuf) -> Result<(), SbatchmanConfigError> {
  let path = path.join(".sbatchman");
  std::fs::create_dir_all(&path).map_err(SbatchmanConfigError::FilesystemError)?;
  init_sbatchman_config_local(&path)?;
  Database::new(&path)?;
  Ok(())
}

/// Searches for the .sbatchman directory starting from the current working directory
/// and moving up the directory tree until it finds it or reaches the user's home directory.
pub fn get_sbatchman_dir() -> Result<PathBuf, SbatchmanConfigError> {
  let home = dirs::home_dir().unwrap_or(PathBuf::from("/"));
  let start = std::env::current_dir().map_err(SbatchmanConfigError::FilesystemError)?;
  let mut dir = start.clone();

  loop {
    let candidate = dir.join(".sbatchman");
    if candidate.is_dir() {
      return Ok(candidate);
    }
    // Stop if we reach the home directory
    if dir == home {
      break;
    }
    // Stop if we reach the root directory
    if !dir.pop() {
      break;
    }
  }

  Err(SbatchmanConfigError::SbatchmanDirNotFound)
}

pub fn get_sbatchman_config_global() -> Result<SbatchmanConfig, SbatchmanConfigError> {
  let config: SbatchmanConfig =
    confy::load("sbatchman", "config").map_err(|e| SbatchmanConfigError::ConfyError(e))?;
  Ok(config)
}

pub fn get_sbatchman_config_local(path: &PathBuf) -> Result<SbatchmanConfig, SbatchmanConfigError> {
  if !path.join("sbatchman.conf").is_file() {
    return Err(SbatchmanConfigError::SbatchmanConfigNotFound);
  }
  let config: SbatchmanConfig = confy::load_path(path.join("sbatchman.conf"))
    .map_err(|e| SbatchmanConfigError::ConfyError(e))?;
  Ok(config)
}

pub fn init_sbatchman_config_global() -> Result<(), SbatchmanConfigError> {
  let cfg_path = confy::get_configuration_file_path("sbatchman", "config").map_err(|e| SbatchmanConfigError::ConfyError(e))?;
  if !cfg_path.exists() {
    let config: SbatchmanConfig = SbatchmanConfig::default();
    confy::store("sbatchman", "config", config).map_err(|e| SbatchmanConfigError::ConfyError(e))?;
    println!("Created global configuration file at '{:?}'", cfg_path);
  }
  Ok(())
}

pub fn init_sbatchman_config_local(path: &PathBuf) -> Result<(), SbatchmanConfigError> {
  let config: SbatchmanConfig = SbatchmanConfig::default();
  confy::store_path(path.join("sbatchman.conf"), config)
    .map_err(|e| SbatchmanConfigError::ConfyError(e))?;
  Ok(())
}

pub fn set_sbatchman_config_global(config: &SbatchmanConfig) -> Result<(), SbatchmanConfigError> {
  confy::store("sbatchman", "config", config).map_err(|e| SbatchmanConfigError::ConfyError(e))?;
  Ok(())
}

pub fn set_sbatchman_config_local(
  path: &PathBuf,
  config: &SbatchmanConfig,
) -> Result<(), SbatchmanConfigError> {
  confy::store_path(path.join("sbatchman.conf"), config)
    .map_err(|e| SbatchmanConfigError::ConfyError(e))?;
  Ok(())
}
