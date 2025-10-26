pub mod models;
pub mod schema;

#[cfg(test)]
mod tests;

use diesel::prelude::*;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::debug;
use std::{collections::HashMap, io, path::Path};
use thiserror::Error;

use crate::core::database::models::{NewClusterConfig, Status};

use super::database::{
  models::{Cluster, Config, NewCluster, NewConfig},
  schema::clusters,
};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Error, Debug)]
pub enum StorageError {
  #[error("Could not read current directory")]
  CurrentDir(#[from] io::Error),
  #[error("Could not connect to database: {0}")]
  ConnectionError(#[from] diesel::ConnectionError),
  #[error("Database migration error: {0}")]
  MigrationError(#[from] Box<dyn std::error::Error + Send + Sync>),
  #[error("Database operation error: {0}")]
  OperationError(String),
}

pub struct Database {
  conn: SqliteConnection,
}

impl Database {
  pub fn new(path: &Path) -> Result<Self, StorageError> {
    let path = path.join("sbatchman.db");
    let database_url = path.to_str().unwrap();
    let mut conn =
      SqliteConnection::establish(&database_url).map_err(StorageError::ConnectionError)?;
    let _ = conn
      .run_pending_migrations(MIGRATIONS)
      .map_err(StorageError::MigrationError)?;

    debug!("Connected to database at {}.", database_url);
    Ok(Database { conn })
  }

  pub fn create_cluster(&mut self, new_cluster: &NewCluster) -> Result<Cluster, StorageError> {
    let cluster = diesel::insert_into(clusters::table)
      .values(new_cluster)
      .returning(Cluster::as_returning())
      .get_result(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;
    Ok(cluster)
  }

  pub fn create_cluster_config(&mut self, new_config: &NewConfig) -> Result<Config, StorageError> {
    use self::schema::configs;

    let config = diesel::insert_into(configs::table)
      .values(new_config)
      .returning(Config::as_returning())
      .get_result(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;
    Ok(config)
  }

  /// Create a cluster along with its associated configurations
  /// Leave cluster_id fields in configs as 0; they will be updated by this function
  /// TODO: add option to allow overwriting existing clusters/configs
  pub fn create_cluster_with_configs(
    &mut self,
    cluster_config: &mut NewClusterConfig,
  ) -> Result<(), StorageError> {
    let cluster = self.create_cluster(&cluster_config.cluster)?;

    cluster_config.configs.iter_mut().for_each(|config| {
      config.cluster_id = cluster.id;
      let _ = self.create_cluster_config(config);
    });
    Ok(())
  }

  pub fn create_job(
    &mut self,
    new_job: &models::NewJob,
  ) -> Result<super::database::models::Job, StorageError> {
    use self::schema::jobs;

    let job = diesel::insert_into(jobs::table)
      .values(new_job)
      .returning(super::database::models::Job::as_returning())
      .get_result(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;
    Ok(job)
  }

  pub fn update_job_path(&mut self, id: i32, directory: &str) -> Result<(), StorageError> {
    use self::schema::jobs::dsl as jobs_dsl;

    diesel::update(jobs_dsl::jobs.filter(jobs_dsl::id.eq(id)))
      .set(jobs_dsl::directory.eq(directory))
      .execute(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;
    Ok(())
  }

  pub fn update_job_status(&mut self, id: i32, new_status: &Status) -> Result<(), StorageError> {
    use self::schema::jobs::dsl as jobs_dsl;

    diesel::update(jobs_dsl::jobs.filter(jobs_dsl::id.eq(id)))
      .set(jobs_dsl::status.eq(new_status))
      .execute(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;
    Ok(())
  }

  pub fn get_cluster_by_name(&mut self, name: &str) -> Result<Cluster, StorageError> {
    use self::schema::clusters::dsl::*;

    let cluster = clusters
      .filter(cluster_name.eq(name))
      .first::<Cluster>(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;
    Ok(cluster)
  }

  pub fn get_cluster_by_id(&mut self, cluster_id: i32) -> Result<Cluster, StorageError> {
    use self::schema::clusters::dsl::*;

    let cluster = clusters
      .filter(id.eq(cluster_id))
      .first::<Cluster>(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;
    Ok(cluster)
  }

  /// Retrieve all configs for a given cluster as a HashMap
  pub fn get_configs_by_cluster(
    &mut self,
    cluster: &Cluster,
  ) -> Result<HashMap<String, Config>, StorageError> {
    let configs_list = Config::belonging_to(&cluster)
      .select(Config::as_select())
      .load(&mut self.conn)
      .map_err(|e| StorageError::OperationError(e.to_string()))?;

    let mut configs_map = HashMap::with_capacity(configs_list.len());
    for config in configs_list {
      configs_map.insert(config.config_name.clone(), config);
    }
    Ok(configs_map)
  }
}
