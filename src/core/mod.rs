mod storage;
mod parsers;
mod config;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use diesel::SqliteConnection;

use storage::create_cluster_with_configs;

pub struct Sbatchman {
  db: SqliteConnection,
  path: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum SbatchmanError {
  #[error("Storage Error: {0}")]
  StorageError(#[from] storage::StorageError),
  #[error("Parser Error: {0}")]
  ParserError(#[from] parsers::ParserError),
  #[error("Config Error: {0}")]
  ConfigError(#[from] config::ConfigError),
}

impl Sbatchman {
  pub fn new() -> Result<Self, SbatchmanError> {
    let _ = env_logger::try_init();

    let path = storage::get_sbatchman_path()?;
    let db = storage::establish_connection(path.clone())?;
    Ok(Sbatchman { db, path })
  }

  pub fn set_cluster_name(&mut self, name: &str) -> Result<(), SbatchmanError> {
    config::set_cluster_name(&self.path, name)?;
    Ok(())
  }

  pub fn import_clusters_configs_from_file(&mut self, path: &str) -> Result<(), SbatchmanError> {
    let mut clusters = parsers::parse_clusters_configs_from_file(path)?;
    for cluster in &mut clusters {
      create_cluster_with_configs(&mut self.db, cluster)?;
    }

    return Ok(());
  }


}