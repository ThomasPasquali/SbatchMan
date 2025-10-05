pub mod models;
pub mod schema;

use diesel::prelude::*;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use dotenvy::dotenv;
use std::{env, str::FromStr};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn establish_connection() -> SqliteConnection {
  dotenv().ok();

  let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
  let mut connection = SqliteConnection::establish(&database_url)
    .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));
  let _ = connection.run_pending_migrations(MIGRATIONS);
  return connection;
}

use super::database::{
  models::{NewCluster, Scheduler},
  schema::clusters,
};

use self::models::Cluster;

pub fn create_cluster(conn: &mut SqliteConnection, cluster_name: &str, scheduler: &str, max_jobs: Option<i32>) -> Cluster {
  let new_cluster = NewCluster {
    cluster_name: cluster_name,
    scheduler: Scheduler::from_str(scheduler).unwrap(),
    max_jobs,
  };

  diesel::insert_into(clusters::table)
    .values(&new_cluster)
    .returning(Cluster::as_returning())
    .get_result(conn)
    .expect("Error saving new cluster")
}

pub fn list_clusters(conn: &mut SqliteConnection) -> Vec<Cluster> {
  use self::schema::clusters::dsl::*;
  clusters
    .load::<Cluster>(conn)
    .expect("Error loading clusters")
}