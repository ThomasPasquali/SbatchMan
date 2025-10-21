use thiserror::Error;

use crate::core::database::Database;

#[derive(Error, Debug)]
pub enum ClusterConfigError {
  #[error("Storage Error: {0}")]
  StorageError(#[from] crate::core::database::StorageError),
}

pub fn create_cluster_with_configs(db: &mut Database, cluster: &ClusterConfig) -> Result<(), ClusterConfigError> {
  db.create_cluster(&cluster.name, &cluster.configs)?;
  Ok(())
}