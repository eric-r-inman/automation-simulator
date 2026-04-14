//! Simulated implementations of [`Controller`] and [`SensorSource`].
//!
//! Both wrap an `Arc<Mutex<SimWorld>>` so a single simulator state
//! serves both trait surfaces — opening a valve through the
//! controller side shows up in the moisture readings on the sensor
//! side without any cross-wiring at the call site.  That matches
//! the real-hardware layout where one set of cables carries both
//! commands and telemetry.
//!
//! The Mutex is `tokio::sync::Mutex` because the trait methods are
//! `async`; a `std::sync::Mutex` would work in v0.1 (no .await
//! across locks) but would bite us the first time a real driver
//! needs an HTTP call inside a locked section.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::controller::{Controller, ZoneStatus};
use super::errors::{ControllerError, SensorError};
use super::sensor::{ReadingKind, SensorReading, SensorSource};
use crate::engine::clock::{SimDuration, SimInstant};
use crate::engine::weather::WeatherSample;
use crate::engine::world::SimWorld;
use crate::sim::id::ZoneId;

/// Shared handle to a simulated world.  Cloning this type only
/// clones the `Arc`, so the controller and sensor source can hold
/// the same world without re-creating it.
#[derive(Clone)]
pub struct SharedWorld(pub Arc<Mutex<SimWorld>>);

impl SharedWorld {
  pub fn new(world: SimWorld) -> Self {
    Self(Arc::new(Mutex::new(world)))
  }
}

impl std::fmt::Debug for SharedWorld {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // Avoid acquiring the mutex for a debug print; the inner world
    // is large and printing it is rarely what the caller wants.
    f.debug_struct("SharedWorld").finish_non_exhaustive()
  }
}

// ── Controller ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SimulatedController {
  world: SharedWorld,
}

impl SimulatedController {
  pub fn new(world: SharedWorld) -> Self {
    Self { world }
  }
}

#[async_trait]
impl Controller for SimulatedController {
  async fn list_zones(&self) -> Result<Vec<ZoneStatus>, ControllerError> {
    let guard = self.world.0.lock().await;
    let now = guard.clock.now();
    // Walk the zones in their stable fixture order so the response
    // is deterministic for snapshot tests and UI diffs.
    let statuses = guard
      .zones
      .iter()
      .map(|z| {
        let v = guard.valves.get(&z.id).copied().unwrap_or_default();
        ZoneStatus {
          zone_id: z.id.clone(),
          is_open: v.is_open(now),
          open_until: v.open_until,
          total_open_seconds: v.total_open_seconds,
        }
      })
      .collect();
    Ok(statuses)
  }

  async fn open_zone(
    &self,
    zone_id: &ZoneId,
    duration: SimDuration,
  ) -> Result<(), ControllerError> {
    let mut guard = self.world.0.lock().await;
    guard.open_zone(zone_id, duration).map_err(|source| {
      // SimWorldError::UnknownZone → ControllerError::ZoneNotFound;
      // any other variant comes back with a context message.
      use crate::engine::world::SimWorldError;
      match source {
        SimWorldError::UnknownZone(z) => ControllerError::ZoneNotFound(z),
        other => ControllerError::ZoneOpen {
          zone: zone_id.clone(),
          reason: other.to_string(),
        },
      }
    })
  }

  async fn close_zone(&self, zone_id: &ZoneId) -> Result<(), ControllerError> {
    let mut guard = self.world.0.lock().await;
    guard.close_zone(zone_id).map_err(|source| {
      use crate::engine::world::SimWorldError;
      match source {
        SimWorldError::UnknownZone(z) => ControllerError::ZoneNotFound(z),
        other => ControllerError::ZoneClose {
          zone: zone_id.clone(),
          reason: other.to_string(),
        },
      }
    })
  }
}

// ── SensorSource ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SimulatedSensorSource {
  world: SharedWorld,
}

impl SimulatedSensorSource {
  pub fn new(world: SharedWorld) -> Self {
    Self { world }
  }
}

#[async_trait]
impl SensorSource for SimulatedSensorSource {
  async fn latest_reading(
    &self,
    zone_id: &ZoneId,
  ) -> Result<Option<SensorReading>, SensorError> {
    let guard = self.world.0.lock().await;
    // Scan history back-to-front so we find the most-recent sample
    // for this zone without sorting.
    let reading = guard
      .history
      .iter()
      .rev()
      .find(|s| &s.zone_id == zone_id)
      .map(|s| SensorReading {
        zone_id: s.zone_id.clone(),
        kind: ReadingKind::SoilVwc,
        value: s.soil_vwc,
        taken_at: SimInstant::from_minutes(s.instant_minutes),
      });
    Ok(reading)
  }

  async fn history(
    &self,
    zone_id: &ZoneId,
    since: SimInstant,
  ) -> Result<Vec<SensorReading>, SensorError> {
    let guard = self.world.0.lock().await;
    let readings = guard
      .history
      .iter()
      .filter(|s| &s.zone_id == zone_id)
      .filter(|s| s.instant_minutes >= since.minutes())
      .map(|s| SensorReading {
        zone_id: s.zone_id.clone(),
        kind: ReadingKind::SoilVwc,
        value: s.soil_vwc,
        taken_at: SimInstant::from_minutes(s.instant_minutes),
      })
      .collect();
    Ok(readings)
  }

  async fn weather_now(&self) -> Result<WeatherSample, SensorError> {
    let mut guard = self.world.0.lock().await;
    let now = guard.clock.now();
    // weather.sample_at takes &mut because the stochastic rain
    // planner memoizes per month on first access; subsequent calls
    // for the same month do no extra work.
    let clock = guard.clock.clone();
    let sample = guard.weather.sample_at(&clock, now);
    Ok(sample)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::catalog::{Catalog, EmitterShape, EmitterSpec, SoilType};
  use crate::engine::world::SimWorld;
  use crate::sim::id::{
    EmitterSpecId, ManifoldInstanceId, SoilTypeId, YardId, ZoneId,
  };
  use crate::sim::zone::{PlantKind, Zone};
  use chrono::NaiveDate;
  use std::sync::Arc as StdArc;

  fn test_world() -> SimWorld {
    let mut cat = Catalog::default();
    cat.soil_types.insert(
      SoilTypeId::new("silty-clay-loam"),
      SoilType {
        id: SoilTypeId::new("silty-clay-loam"),
        name: "Silty Clay Loam".into(),
        saturation_vwc: 0.48,
        field_capacity_vwc: 0.36,
        wilting_point_vwc: 0.17,
        saturated_hydraulic_conductivity_mm_per_hr: 3.5,
        notes: None,
      },
    );
    cat.emitters.insert(
      EmitterSpecId::new("inline-drip-12in"),
      EmitterSpec {
        id: EmitterSpecId::new("inline-drip-12in"),
        name: "Inline Drip".into(),
        manufacturer: "Example".into(),
        price_usd_per_100: 30.0,
        shape: EmitterShape::InlineDrip,
        flow_gph: 0.9,
        min_inlet_psi: 15.0,
        pressure_compensating: true,
        inline_spacing_inches: Some(12.0),
        notes: None,
      },
    );
    let zone = Zone {
      id: ZoneId::new("z1"),
      yard_id: YardId::new("y"),
      manifold_id: ManifoldInstanceId::new("m"),
      plant_kind: PlantKind::VeggieBed,
      emitter_spec_id: EmitterSpecId::new("inline-drip-12in"),
      soil_type_id: SoilTypeId::new("silty-clay-loam"),
      area_sq_ft: 50.0,
      notes: None,
    };
    SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![zone],
      StdArc::new(cat),
      1,
      0.20,
      Vec::new(),
    )
    .expect("build world")
  }

  #[tokio::test]
  async fn opening_a_zone_through_the_controller_raises_moisture() {
    use crate::engine::clock::SimDuration;
    let shared = SharedWorld::new(test_world());
    let controller = SimulatedController::new(shared.clone());
    let sensors = SimulatedSensorSource::new(shared.clone());
    let zone = ZoneId::new("z1");

    controller
      .open_zone(&zone, SimDuration::minutes(10))
      .await
      .expect("open");

    // Advance 30 minutes through the shared world; the controller
    // and sensor source both observe it.
    {
      let mut guard = shared.0.lock().await;
      guard.advance(SimDuration::minutes(30));
    }

    // list_zones reflects that some open time was logged.
    let zones = controller.list_zones().await.expect("list");
    assert_eq!(zones.len(), 1);
    assert!(
      zones[0].total_open_seconds >= 600,
      "expected at least 10 minutes of open time, got {}",
      zones[0].total_open_seconds
    );

    // The sensor source should show a moisture reading higher than
    // the 0.20 starting VWC.
    let latest = sensors
      .latest_reading(&zone)
      .await
      .expect("latest")
      .expect("present");
    assert!(
      latest.value > 0.20,
      "expected moisture to rise above 0.20 after irrigation, got {}",
      latest.value
    );
  }

  #[tokio::test]
  async fn unknown_zone_returns_semantic_error() {
    let shared = SharedWorld::new(test_world());
    let controller = SimulatedController::new(shared);
    let err = controller
      .open_zone(
        &ZoneId::new("ghost"),
        crate::engine::clock::SimDuration::minutes(1),
      )
      .await
      .unwrap_err();
    assert!(matches!(err, ControllerError::ZoneNotFound(_)));
  }

  #[tokio::test]
  async fn status_joins_controller_and_zone_snapshots() {
    let shared = SharedWorld::new(test_world());
    let controller = SimulatedController::new(shared);
    let status = controller.status().await.expect("status");
    assert_eq!(status.zones.len(), 1);
  }

  #[tokio::test]
  async fn weather_now_returns_a_sample() {
    let shared = SharedWorld::new(test_world());
    let sensors = SimulatedSensorSource::new(shared);
    let w = sensors.weather_now().await.expect("weather");
    // Portland July mean is ~21 C; the stochastic layer may nudge
    // it but not outside a plausible summer range.
    assert!((10.0..40.0).contains(&w.temperature_c));
  }
}
