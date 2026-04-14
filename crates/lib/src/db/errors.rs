//! Semantic errors for the persistence layer.  The module's callers
//! distinguish "failed to open the database" from "failed to apply a
//! migration" from "a query did not produce the expected shape";
//! each has its own variant so the server and CLI can tailor the
//! response without string-matching.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbOpenError {
  #[error("failed to open SQLite database at {path:?}: {source}")]
  Connect {
    path: PathBuf,
    #[source]
    source: sqlx::Error,
  },

  #[error(
    "failed to create parent directory {path:?} before opening the database: {source}"
  )]
  CreateParentDir {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },

  #[error("SQLite URL {url:?} is malformed: {reason}")]
  InvalidUrl { url: String, reason: String },
}

#[derive(Debug, Error)]
pub enum MigrationError {
  #[error("failed to apply schema migrations: {0}")]
  Apply(#[from] sqlx::migrate::MigrateError),
}

/// Errors produced by the typed query helpers.  Query-level errors
/// carry the operation that failed so the caller reports "inserting
/// a plant failed" rather than a bare SQLite error.
#[derive(Debug, Error)]
pub enum QueryError {
  #[error("database query '{operation}' failed: {source}")]
  Sqlx {
    operation: &'static str,
    #[source]
    source: sqlx::Error,
  },

  #[error("failed to serialize '{field}' to JSON before insert: {source}")]
  JsonEncode {
    field: &'static str,
    #[source]
    source: serde_json::Error,
  },

  #[error("failed to deserialize '{field}' from JSON after select: {source}")]
  JsonDecode {
    field: &'static str,
    #[source]
    source: serde_json::Error,
  },
}

impl QueryError {
  pub fn sqlx(operation: &'static str, source: sqlx::Error) -> Self {
    Self::Sqlx { operation, source }
  }
}
