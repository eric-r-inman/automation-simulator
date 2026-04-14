//! Semantic errors for the hardware trait layer.
//!
//! The trait callers (server handlers, CLI subcommands) match on
//! these variants to decide HTTP status and diagnostic text.  Each
//! variant names the operation that failed and, where useful, the
//! offending zone id — enough context to report a failure without
//! string-matching the inner source error.

use thiserror::Error;

use crate::sim::id::ZoneId;

/// Errors produced by [`crate::hw::Controller`] implementations.
#[derive(Debug, Error)]
pub enum ControllerError {
  #[error("controller has no zone with id {0}")]
  ZoneNotFound(ZoneId),

  #[error("failed to open zone {zone}: {reason}")]
  ZoneOpen { zone: ZoneId, reason: String },

  #[error("failed to close zone {zone}: {reason}")]
  ZoneClose { zone: ZoneId, reason: String },

  #[error("controller is unreachable: {0}")]
  Unreachable(String),

  /// Real-hardware drivers in v0.2 will surface transient network
  /// errors this way; simulated controllers never produce it.
  #[error("controller timed out after {seconds}s waiting for {operation}")]
  Timeout {
    operation: &'static str,
    seconds: u64,
  },
}

/// Errors produced by [`crate::hw::SensorSource`] implementations.
#[derive(Debug, Error)]
pub enum SensorError {
  #[error("sensor source has no data for zone {0}")]
  ZoneNotFound(ZoneId),

  #[error("sensor source is unreachable: {0}")]
  Unreachable(String),

  #[error("sensor source timed out after {seconds}s waiting for {operation}")]
  Timeout {
    operation: &'static str,
    seconds: u64,
  },
}
