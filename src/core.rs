mod cluster_configs;
pub mod database;
pub mod jobs;
mod parsers;
pub mod sbatchman_configs;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use crate::core::database::Database;

pub struct Sbatchman {
  db: Database,
  path: PathBuf,
  config_global: sbatchman_configs::SbatchmanConfig,
  config_local: sbatchman_configs::SbatchmanConfig,
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
    let config_global = sbatchman_configs::get_sbatchman_config_global()?;
    let config_local = sbatchman_configs::get_sbatchman_config_local(&path)?;
    Ok(Sbatchman {
      db,
      path,
      config_global,
      config_local,
    })
  }

  pub fn init(path: &PathBuf) -> Result<(), SbatchmanError> {
    sbatchman_configs::init_sbatchman_dir(path)?;
    sbatchman_configs::init_sbatchman_config_global()?;
    Ok(())
  }

  pub fn set_cluster_name(&mut self, name: &str, local: bool) -> Result<(), SbatchmanError> {
    if local {
      self.config_global.cluster_name = Some(name.to_string());
      sbatchman_configs::set_sbatchman_config_global(&mut self.config_global)?;
    } else {
      self.config_global.cluster_name = Some(name.to_string());
      sbatchman_configs::set_sbatchman_config_local(&self.path, &mut self.config_global)?;
    }
    Ok(())
  }

  pub fn get_cluster_name(&self) -> Option<String> {
    self
      .get_cluster_name_local()
      .or_else(|| self.get_cluster_name_global())
  }

  pub fn get_cluster_name_global(&self) -> Option<String> {
    self.config_global.cluster_name.clone()
  }

  pub fn get_cluster_name_local(&self) -> Option<String> {
    self.config_local.cluster_name.clone()
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
        .config_global
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
