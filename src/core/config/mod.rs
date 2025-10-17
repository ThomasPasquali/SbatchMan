use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
pub mod tests;

#[derive(Error, Debug)]
pub enum ConfigError {
  #[error("Configuration error: {0}")]
  ConfigError(String),
}

#[derive(Serialize, Deserialize)]
struct SbatchmanConfig {
  cluster_name: String,
}
impl ::std::default::Default for SbatchmanConfig {
  fn default() -> Self {
    Self {
      cluster_name: "".into(),
    }
  }
}

pub fn sbatchman_init(path: &PathBuf) -> Result<(), ConfigError> {
  let config: SbatchmanConfig = SbatchmanConfig::default();
  confy::store_path(path.join("sbatchman.conf"), config)
    .map_err(|e| ConfigError::ConfigError(e.to_string()))?;
  Ok(())
}

pub fn set_cluster_name(path: &PathBuf, name: &str) -> Result<(), ConfigError> {
  let mut config: SbatchmanConfig = confy::load_path(path.join("sbatchman.conf"))
    .map_err(|e| ConfigError::ConfigError(e.to_string()))?;
  config.cluster_name = name.to_string();
  confy::store_path(path.join("sbatchman.conf"), config)
    .map_err(|e| ConfigError::ConfigError(e.to_string()))?;
  Ok(())
}

pub fn get_cluster_name(path: &PathBuf) -> Result<String, ConfigError> {
  let config: SbatchmanConfig = confy::load_path(path.join("sbatchman.conf"))
    .map_err(|e| ConfigError::ConfigError(e.to_string()))?;
  Ok(config.cluster_name)
}
