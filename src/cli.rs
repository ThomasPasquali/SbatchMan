mod utils;
use std::env;

use crate::core::{self, Sbatchman};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
  Init {},
  Configure {
    file: String,
  },
  Update {},
  SetClusterName {
    name: String,
  },
  Launch {
    file: String,
    cluster_name: Option<String>,
  },
}

pub fn main() {
  let cli = Cli::parse();

  if let Some(Commands::Init {}) = &cli.command {
  } else {
    match &cli.command {
      Some(Commands::Configure { file }) => {
        let mut sbatchman = core::Sbatchman::new().expect("Failed to initialize Sbatchman");
        sbatchman
          .import_clusters_configs_from_file(file)
          .expect("Failed to import clusters and configs from file");
      }
      Some(Commands::Init {}) => {
        let path = env::current_dir().expect("Failed to get current directory");
        Sbatchman::init(&path).expect("Failed to initialize sbatchman directory");
        println!("✅ Sbatchman initialized successfully!");
      }
      Some(Commands::Update {}) => {
        utils::update().expect("Failed to update sbatchman");
      }
      Some(Commands::SetClusterName { name }) => {
        let mut sbatchman = core::Sbatchman::new().expect("Failed to initialize Sbatchman");
        sbatchman
          .set_cluster_name(name)
          .expect("Failed to set cluster name in sbatchman configuration");
        println!("✅ Cluster name set to '{}' successfully!", name);
      }
      Some(Commands::Launch {
        file,
        cluster_name: cluster,
      }) => {
        let mut sbatchman = core::Sbatchman::new().expect("Failed to initialize Sbatchman");
        sbatchman
          .launch_jobs_from_file(file, cluster)
          .expect("Failed to launch jobs from file");
      }
      None => {}
    }
  }
}
