//! Controller trait.
//!
//! [`Controller`] is the abstraction the simulator and the real-
//! hardware drivers share.  v0.1 ships only a [`super::simulated::SimulatedController`];
//! v0.2 will add a driver that speaks HTTP to physical hardware,
//! and v0.3 picks one at runtime per the `hardware_mode` flag.
//!
//! The trait is `async_trait` so `Arc<dyn Controller>` works in the
//! server's `AppState`.  All methods borrow `&self` — interior
//! mutability is the impl's concern — so the callers never have to
//! pass `&mut` around.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::errors::ControllerError;
use crate::engine::clock::{SimDuration, SimInstant};
use crate::sim::id::ZoneId;

/// Observable state of a single zone as reported by a controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneStatus {
  pub zone_id: ZoneId,
  pub is_open: bool,
  /// When the valve is currently scheduled to close.  `None` while
  /// the valve is closed or the underlying controller does not
  /// expose a planned-close instant.
  pub open_until: Option<SimInstant>,
  /// Cumulative seconds this zone has been open across its lifetime.
  pub total_open_seconds: i64,
}

/// Controller-wide status snapshot.  Separate from [`ZoneStatus`]
/// because the server exposes it at `GET /api/zones` alongside the
/// per-zone data, and real controllers typically report both in
/// one API call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControllerStatus {
  pub zones: Vec<ZoneStatus>,
}

#[async_trait]
pub trait Controller: Send + Sync {
  /// Snapshot of every zone the controller knows about.
  async fn list_zones(&self) -> Result<Vec<ZoneStatus>, ControllerError>;

  /// Open a zone's valve for `duration`.  A second open while the
  /// valve is already open extends the run to the later of the
  /// two scheduled close instants; the impl is responsible for
  /// that policy.
  async fn open_zone(
    &self,
    zone_id: &ZoneId,
    duration: SimDuration,
  ) -> Result<(), ControllerError>;

  /// Close a zone's valve immediately.  No-op if already closed.
  async fn close_zone(&self, zone_id: &ZoneId) -> Result<(), ControllerError>;

  /// Full status snapshot — convenience wrapper around
  /// `list_zones` plus any controller-level fields a later impl
  /// wants to report.
  async fn status(&self) -> Result<ControllerStatus, ControllerError> {
    Ok(ControllerStatus {
      zones: self.list_zones().await?,
    })
  }
}
