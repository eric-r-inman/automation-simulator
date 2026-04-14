//! Sensor source trait.
//!
//! [`SensorSource`] abstracts over "where a zone's moisture reading
//! comes from".  v0.1 ships only [`super::simulated::SimulatedSensorSource`],
//! but the trait is shaped so an HTTP-push receiver in v0.2 drops
//! in without any changes to the consumers — the server routes and
//! the dashboard talk to the trait, not any specific impl.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::errors::SensorError;
use crate::engine::clock::SimInstant;
use crate::engine::weather::WeatherSample;
use crate::sim::id::ZoneId;

/// What a single reading measured.  Kept compact — richer payloads
/// (flow, pressure) land as additional variants when the hardware
/// that produces them is supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadingKind {
  SoilVwc,
  Temperature,
  Flow,
}

/// One sensor reading at one instant.  Values are in SI units for
/// the temperature and flow variants, and a dimensionless ratio
/// `[0, 1]` for `SoilVwc`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorReading {
  pub zone_id: ZoneId,
  pub kind: ReadingKind,
  pub value: f64,
  pub taken_at: SimInstant,
}

#[async_trait]
pub trait SensorSource: Send + Sync {
  /// Most recent reading for this zone, if any.  Returns `Ok(None)`
  /// when the source simply has no data yet (as opposed to an
  /// unreachable source, which returns `Err(Unreachable)`).
  async fn latest_reading(
    &self,
    zone_id: &ZoneId,
  ) -> Result<Option<SensorReading>, SensorError>;

  /// All readings for this zone at or after `since`.  Returned in
  /// increasing instant order.
  async fn history(
    &self,
    zone_id: &ZoneId,
    since: SimInstant,
  ) -> Result<Vec<SensorReading>, SensorError>;

  /// Current weather at the property.  Same caveat as
  /// `latest_reading`: `Err(Unreachable)` is a transport problem,
  /// not "no data".
  async fn weather_now(&self) -> Result<WeatherSample, SensorError>;
}
