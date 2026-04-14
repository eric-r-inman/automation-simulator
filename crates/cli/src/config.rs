//! Global CLI options.
//!
//! Only log-level and log-format live here now; everything else is
//! carried by the specific subcommand's arg struct.  The command
//! dispatcher lives in `main.rs`.

use automation_simulator_lib::{LogFormat, LogLevel};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
  #[error("invalid log level: {0}")]
  InvalidLogLevel(String),

  #[error("invalid log format: {0}")]
  InvalidLogFormat(String),
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
  /// Log level (trace, debug, info, warn, error)
  #[arg(long, global = true, env = "LOG_LEVEL")]
  pub log_level: Option<String>,

  /// Log format (text, json)
  #[arg(long, global = true, env = "LOG_FORMAT")]
  pub log_format: Option<String>,

  #[command(subcommand)]
  pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
  /// Load a property TOML fixture into a fresh SQLite database.
  Seed {
    /// Path to the property fixture TOML file.
    #[arg(long)]
    property: PathBuf,

    /// Path to the catalog directory (contains
    /// `controllers.toml`, `species.toml`, ...).
    #[arg(long, default_value = "data/catalog")]
    catalog: PathBuf,

    /// Path to the SQLite database file.  Created if missing.
    #[arg(long)]
    db: PathBuf,
  },
}

#[derive(Debug)]
pub struct Config {
  pub log_level: LogLevel,
  pub log_format: LogFormat,
}

impl Config {
  pub fn from_cli(cli: &Cli) -> Result<Self, ConfigError> {
    let log_level = cli
      .log_level
      .as_deref()
      .unwrap_or("info")
      .parse::<LogLevel>()
      .map_err(|e| ConfigError::InvalidLogLevel(e.to_string()))?;
    let log_format = cli
      .log_format
      .as_deref()
      .unwrap_or("text")
      .parse::<LogFormat>()
      .map_err(|e| ConfigError::InvalidLogFormat(e.to_string()))?;
    Ok(Self {
      log_level,
      log_format,
    })
  }
}
