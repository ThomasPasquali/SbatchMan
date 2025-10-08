use std::fmt::Display;

use super::schema::{clusters, configs, jobs};
use diesel::{
  backend::Backend,
  deserialize::{FromSql, FromSqlRow},
  expression::AsExpression,
  prelude::*,
  serialize::{Output, ToSql},
  sql_types::Integer,
};
use strum::EnumString;

#[repr(i32)]
#[derive(FromSqlRow, Debug, AsExpression, EnumString)]
#[diesel(sql_type = Integer)]
pub enum Scheduler {
  Local = 0,
  Slurm = 1,
  PBS = 2,
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
      2 => Ok(Scheduler::PBS),
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
      Scheduler::PBS => 2.to_sql(out),
    }
  }
}

impl Display for Scheduler {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let s = match self {
      Scheduler::Local => "Local",
      Scheduler::Slurm => "Slurm",
      Scheduler::PBS => "PBS",
    };
    write!(f, "{}", s)
  }
}

#[derive(Queryable, Selectable)]
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

#[derive(Queryable, Selectable, Associations, Debug, PartialEq)]
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

pub struct ConfigWithCluster {
  pub config: Config,
  pub cluster: Cluster,
}

#[derive(Queryable, Selectable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(Config))]
#[diesel(table_name = jobs)]
pub struct Job {
  pub id: i32,
  pub job_name: String,
  pub config_id: i32,
  pub submit_time: i32,
  pub directory: String,
  pub command: String,
  pub status: String,
  pub job_id: Option<String>,
  pub end_time: Option<i32>,
  pub preprocess: Option<String>,
  pub postprocess: Option<String>,
  pub archived: Option<i32>,
  pub variables: serde_json::Value,
}

#[derive(Insertable)]
#[diesel(table_name = jobs)]
pub struct NewJob<'a> {
  pub job_name: &'a str,
  pub config_id: i32,
  pub submit_time: i32,
  pub directory: &'a str,
  pub command: &'a str,
  pub status: &'a str,
  pub job_id: Option<&'a str>,
  pub end_time: Option<i32>,
  pub preprocess: Option<&'a str>,
  pub postprocess: Option<&'a str>,
  pub archived: Option<i32>,
  pub variables: &'a serde_json::Value,
}
