//! automation-simulator-cli — command-line entry point.
//!
//! main.rs stays an orchestrator: it parses the top-level `Cli`,
//! builds a `Config` for log settings, initializes logging, then
//! dispatches the selected subcommand.  Business logic lives in
//! `commands/*`.

mod commands;
mod config;
mod logging;

use clap::Parser;
use config::{Cli, Command, Config, ConfigError};
use logging::init_logging;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
enum ApplicationError {
  #[error("configuration error: {0}")]
  Config(#[from] ConfigError),

  #[error("seed command failed: {0}")]
  Seed(#[from] commands::seed::SeedCommandError),
}

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
  let cli = Cli::parse();
  let cfg = Config::from_cli(&cli).map_err(|e| {
    eprintln!("Configuration error: {}", e);
    e
  })?;
  init_logging(cfg.log_level, cfg.log_format);

  match &cli.command {
    Command::Seed {
      property,
      catalog,
      db,
    } => {
      commands::seed::run(property, catalog, db)
        .await
        .map_err(|e| {
          error!(error = %e, "Seed command failed");
          ApplicationError::Seed(e)
        })?;
    }
  }

  Ok(())
}
