use super::schema::{clusters, configs, jobs};
use diesel::{
  backend::Backend,
  deserialize::{FromSql, FromSqlRow},
  expression::AsExpression,
  prelude::*,
  serialize::{Output, ToSql},
  sql_types::Integer,
};
use serde::{Deserialize, Serialize};
use strum::EnumString;

#[repr(i32)]
#[derive(FromSqlRow, Debug, AsExpression, EnumString, PartialEq, Clone)]
#[diesel(sql_type = Integer)]
pub enum Scheduler {
  Local,
  Slurm,
  Pbs,
}

impl<DB> FromSql<Integer, DB> for Scheduler
where
  DB: Backend,
  i32: FromSql<Integer, DB>,
{
  fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
    match i32::from_sql(bytes)? {
      0 => Ok(Scheduler::Local),
      1 => Ok(Scheduler::Slurm),
      2 => Ok(Scheduler::Pbs),
      x => Err(format!("Unrecognized variant {}", x).into()),
    }
  }
}

impl<DB> ToSql<Integer, DB> for Scheduler
where
  DB: Backend,
  i32: ToSql<Integer, DB>,
{
  fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
    match self {
      Scheduler::Local => 0.to_sql(out),
      Scheduler::Slurm => 1.to_sql(out),
      Scheduler::Pbs => 2.to_sql(out),
    }
  }
}

#[derive(Queryable, Selectable, Identifiable)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = clusters)]
pub struct Cluster {
  pub id: i32,
  pub cluster_name: String,
  pub scheduler: Scheduler,
  pub max_jobs: Option<i32>,
}

#[derive(Insertable)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = clusters)]
pub struct NewCluster<'a> {
  pub cluster_name: &'a str,
  pub scheduler: Scheduler,
  pub max_jobs: Option<i32>,
}

#[derive(Queryable, Selectable, Associations, Debug, PartialEq, Identifiable)]
#[diesel(belongs_to(Cluster))]
#[diesel(table_name = configs)]
pub struct Config {
  pub id: i32,
  pub config_name: String,
  pub cluster_id: i32,
  pub flags: serde_json::Value,
  pub env: serde_json::Value,
}

#[derive(Insertable)]
#[diesel(table_name = configs)]
pub struct NewConfig<'a> {
  pub config_name: &'a str,
  pub cluster_id: i32,
  pub flags: &'a serde_json::Value,
  pub env: &'a serde_json::Value,
}

pub struct NewClusterConfig<'a> {
  pub cluster: NewCluster<'a>,
  pub configs: Vec<NewConfig<'a>>,
}

#[repr(i32)]
#[derive(FromSqlRow, Debug, AsExpression, EnumString, PartialEq, Serialize, Deserialize, Clone)]
#[diesel(sql_type = Integer)]
pub enum Status {
  Created,          // Job created but not yet submitted
  VirtualQueue,     // Job in virtual queue waiting for submission
  Queued,           // Job submitted and waiting in scheduler queue
  Running,          // Job is currently running
  Completed,        // Job completed successfully
  Failed,           // Job failed
  Timeout,          // Job timed-out
  FailedSubmission, // Job submission failed
}

impl<DB> FromSql<Integer, DB> for Status
where
  DB: Backend,
  i32: FromSql<Integer, DB>,
{
  fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
    match i32::from_sql(bytes)? {
      0 => Ok(Status::Created),
      1 => Ok(Status::VirtualQueue),
      2 => Ok(Status::Queued),
      3 => Ok(Status::Running),
      4 => Ok(Status::Completed),
      5 => Ok(Status::Failed),
      x => Err(format!("Unrecognized variant {}", x).into()),
    }
  }
}

impl<DB> ToSql<Integer, DB> for Status
where
  DB: Backend,
  i32: ToSql<Integer, DB>,
{
  fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
    match self {
      Status::Created => 0.to_sql(out),
      Status::VirtualQueue => 1.to_sql(out),
      Status::Queued => 2.to_sql(out),
      Status::Running => 3.to_sql(out),
      Status::Completed => 4.to_sql(out),
      Status::Failed => 5.to_sql(out),
      Status::Timeout => 6.to_sql(out),
      Status::FailedSubmission => 7.to_sql(out),
    }
  }
}

#[derive(Queryable, Selectable, Associations, Debug, PartialEq, Serialize, Deserialize, Clone)]
#[diesel(belongs_to(Config))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = jobs)]
pub struct Job {
  pub id: i32,
  pub job_name: String,
  pub config_id: i32,
  pub submit_time: Option<i32>,
  pub directory: String,
  pub command: String,
  pub status: Status,
  pub job_id: Option<String>,
  pub end_time: Option<i32>,
  pub preprocess: Option<String>,
  pub postprocess: Option<String>,
  // pub exit_code: Option<i32>,
  pub archived: Option<i32>,
  pub variables: serde_json::Value,
}

#[derive(Insertable)]
#[diesel(table_name = jobs)]
pub struct NewJob<'a> {
  pub job_name: &'a str,
  pub config_id: i32,
  pub directory: &'a str,
  pub command: &'a str,
  pub status: &'a Status,
  pub preprocess: Option<&'a str>,
  pub postprocess: Option<&'a str>,
  pub variables: &'a serde_json::Value,
}
