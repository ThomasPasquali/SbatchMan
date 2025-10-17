use clap::{Parser, Subcommand};

use crate::core;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
  Configure { file: String },
}

pub fn main() {
  let cli = Cli::parse();

  let mut sbatchman = core::Sbatchman::new().expect("Failed to initialize Sbatchman");

  match &cli.command {
    Some(Commands::Configure { file }) => {
      sbatchman
        .import_clusters_configs_from_file(file)
        .expect("Failed to import clusters and configs from file");
    }
    None => {}
  }
}
