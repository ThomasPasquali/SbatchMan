mod cluster_configs;
mod database;
mod jobs;
mod parsers;
mod sbatchman_configs;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use crate::core::database::Database;

pub struct Sbatchman {
  db: Database,
  path: PathBuf,
  config: sbatchman_configs::SbatchmanConfig,
}

#[derive(thiserror::Error, Debug)]
pub enum SbatchmanError {
  #[error("Storage Error: {0}")]
  StorageError(#[from] database::StorageError),
  #[error("Parser Error: {0}")]
  ParserError(#[from] parsers::ParserError),
  #[error("Config Error: {0}")]
  ConfigError(#[from] sbatchman_configs::SbatchmanConfigError),
  #[error(
    "No cluster set. Please set a cluster before launching jobs using `set-cluster` command."
  )]
  NoClusterSet,
  #[error("Job Error: {0}")]
  JobError(#[from] jobs::JobError),
}

impl Sbatchman {
  pub fn new() -> Result<Self, SbatchmanError> {
    let _ = env_logger::try_init();

    let path = sbatchman_configs::get_sbatchman_dir()?;
    let db = Database::new(&path)?;
    let config = sbatchman_configs::get_sbatchman_config(&path)?;
    Ok(Sbatchman { db, path, config })
  }

  pub fn init(path: &PathBuf) -> Result<(), SbatchmanError> {
    sbatchman_configs::init_sbatchman_dir(path)?;
    Ok(())
  }

  pub fn set_cluster_name(&mut self, name: &str) -> Result<(), SbatchmanError> {
    self.config.cluster_name = Some(name.to_string());
    sbatchman_configs::set_sbatchman_config(&self.path, &mut self.config)?;
    Ok(())
  }

  pub fn import_clusters_configs_from_file(&mut self, path: &str) -> Result<(), SbatchmanError> {
    let mut clusters_configs = parsers::parse_clusters_configs_from_file(&Path::new(path))?;
    for cluster_config in &mut clusters_configs {
      self.db.create_cluster_with_configs(cluster_config)?;
    }

    Ok(())
  }

  pub fn launch_jobs_from_file(
    &mut self,
    path: &str,
    cluster_name: &Option<String>,
  ) -> Result<(), SbatchmanError> {
    let cluster_name = match &cluster_name {
      Some(name) => name,
      None => self
        .config
        .cluster_name
        .as_ref()
        .ok_or(SbatchmanError::NoClusterSet)?,
    };
    Ok(jobs::launch_jobs_from_file(
      &PathBuf::from(path),
      &mut self.db,
      cluster_name,
    )?)
  }
}
