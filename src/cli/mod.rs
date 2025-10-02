use clap::{Parser, Subcommand};

use crate::api;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
  /// does testing things
  AddCluster {
    /// cluster name
    #[arg(short, long)]
    cluster_name: String,

    /// scheduler name
    #[arg(short, long)]
    scheduler: String,
  },
  ListClusters,
}

pub fn main() {
  let cli = Cli::parse();

  let connection = &mut api::establish_connection();

  match &cli.command {
    Some(Commands::AddCluster { cluster_name, scheduler }) => {
      println!("Adding cluster '{}' with scheduler '{}'", cluster_name, scheduler);
      api::create_cluster(connection, cluster_name, scheduler);
    },
    Some(Commands::ListClusters) => {
      let results = api::list_clusters(connection);
      println!("Displaying {} clusters", results.len());
      for cluster in results {
        println!(
          "{}: {} ({})",
          cluster.id, cluster.cluster_name, format!("{:?}", cluster.scheduler)
        );
      }
    }
    None => {}
  }
}