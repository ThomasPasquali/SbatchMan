pub mod models;
pub mod schema;

use diesel::prelude::*;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::debug;
use std::{io, path::PathBuf};
use thiserror::Error;

use super::storage::{
  models::{Cluster, Config, NewCluster, NewConfig},
  schema::clusters,
};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Error, Debug)]
pub enum StorageError {
  #[error("Could not read current directory")]
  CurrentDir(#[from] io::Error),
  #[error("Could not find .sbatchman directory")]
  SbatchmanDirNotFound,
  #[error("Could not connect to database: {0}")]
  ConnectionError(#[from] diesel::ConnectionError),
  #[error("Database migration error: {0}")]
  MigrationError(#[from] Box<dyn std::error::Error + Send + Sync>),
  #[error("Database operation error: {0}")]
  OperationError(String),
}

pub fn get_sbatchman_path() -> Result<PathBuf, StorageError> {
  let home = dirs::home_dir().unwrap_or(PathBuf::from("/"));
  let start = std::env::current_dir().map_err(StorageError::CurrentDir)?;
  let mut dir = start.clone();

  loop {
    let candidate = dir.join(".sbatchman");
    if candidate.is_dir() {
      return Ok(candidate);
    }
    if dir == home {
      break;
    }
    if !dir.pop() {
      break;
    }
  }

  Err(StorageError::SbatchmanDirNotFound)
}

pub fn establish_connection(mut path: PathBuf) -> Result<SqliteConnection, StorageError> {
  path.push("sbatchman.db");
  let database_url = path.to_str().unwrap();
  let mut connection =
    SqliteConnection::establish(&database_url).map_err(StorageError::ConnectionError)?;
  let _ = connection
    .run_pending_migrations(MIGRATIONS)
    .map_err(StorageError::MigrationError)?;

  debug!("Connected to database at {}.", database_url);
  return Ok(connection);
}

pub fn create_cluster(
  conn: &mut SqliteConnection,
  new_cluster: &NewCluster,
) -> Result<Cluster, StorageError> {
  let cluster = diesel::insert_into(clusters::table)
    .values(new_cluster)
    .returning(Cluster::as_returning())
    .get_result(conn)
    .map_err(|e| StorageError::OperationError(e.to_string()))?;
  Ok(cluster)
}

pub fn create_cluster_config(
  conn: &mut SqliteConnection,
  new_config: &NewConfig,
) -> Result<Config, StorageError> {
  use self::schema::configs;

  let config = diesel::insert_into(configs::table)
    .values(new_config)
    .returning(Config::as_returning())
    .get_result(conn)
    .map_err(|e| StorageError::OperationError(e.to_string()))?;
  Ok(config)
}

pub fn create_cluster_with_configs(
  conn: &mut SqliteConnection,
  cluster_config: &mut super::parsers::NewClusterConfig,
) -> Result<(), StorageError> {
  let cluster = create_cluster(conn, &cluster_config.cluster)?;

  cluster_config.configs.iter_mut().for_each(|config| {
    config.cluster_id = cluster.id;
    let _ = create_cluster_config(conn, config);
  });
  Ok(())
}

pub fn get_cluster_config(
  conn: &mut SqliteConnection,
  config_name_: &str,
) -> Result<(Config, Cluster), StorageError> {
  use self::schema::configs::dsl::*;
  let mut config_with_cluster = configs
    .filter(config_name.eq(config_name_))
    .inner_join(clusters::table)
    .select((Config::as_select(), Cluster::as_select()))
    .load::<(Config, Cluster)>(conn)
    .map_err(|e| StorageError::OperationError(e.to_string()))?;
  return Ok(
    config_with_cluster
      .pop()
      .ok_or(StorageError::OperationError("Config not found".into()))?,
  );
}
