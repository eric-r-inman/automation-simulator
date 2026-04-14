//! Top-level simulation world.
//!
//! Owns the clock, the weather model, and the per-zone soil and
//! valve state.  [`SimWorld::advance`] walks a requested span in
//! one-minute sub-steps so the ODE integrator always sees a uniform
//! dt and the resulting time series is reproducible.
//!
//! The world borrows a shared [`Catalog`] by `Arc` — cheap to clone,
//! safe to share across the CLI's single-threaded seed path and the
//! server's multi-request path in later phases.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use super::clock::{SimClock, SimDuration, SimInstant};
use super::flow::zone_inflow_mm_per_hour;
use super::soil::{
  default_root_depth_inches, soil_step, SoilParams, SoilState,
};
use super::weather::{
  reference_et0_mm_per_day, Climatology, WeatherModel, WeatherSample,
};
use crate::catalog::Catalog;
use crate::sim::id::{EmitterSpecId, SoilTypeId, ZoneId};
use crate::sim::scenario::WeatherOverride;
use crate::sim::zone::Zone;

// ── Valve state ──────────────────────────────────────────────────────────────

/// Per-zone valve state.  Open valves carry an absolute end-instant
/// so the scheduler closes them at the right time regardless of the
/// sub-step size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValveState {
  pub open_until: Option<SimInstant>,
  pub total_open_seconds: i64,
}

impl Default for ValveState {
  fn default() -> Self {
    Self {
      open_until: None,
      total_open_seconds: 0,
    }
  }
}

impl ValveState {
  pub fn is_open(&self, now: SimInstant) -> bool {
    match self.open_until {
      Some(t) => now < t,
      None => false,
    }
  }
}

// ── Sensor sample ────────────────────────────────────────────────────────────

/// One row in the simulation's time-series history.  Per-zone
/// moisture readings are flattened into a single vector keyed by
/// (instant, zone_id) so the caller can stream them to a DB or a
/// chart without reshaping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorSample {
  pub instant_minutes: i64,
  pub zone_id: ZoneId,
  pub soil_vwc: f64,
  pub valve_open: bool,
  pub weather_temperature_c: f64,
  pub weather_precipitation_mm_per_hour: f64,
}

// ── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SimWorldError {
  #[error(
    "property climate zone {0:?} is not known to the engine; \
     supply a custom climatology or pick a supported name"
  )]
  UnknownClimateZone(String),

  #[error(
    "zone {0} references soil type {1} but it is absent from the catalog"
  )]
  UnknownSoilType(ZoneId, SoilTypeId),

  #[error(
    "zone {0} references emitter spec {1} but it is absent from the catalog"
  )]
  UnknownEmitterSpec(ZoneId, EmitterSpecId),

  #[error("tried to open/close zone {0} but no such zone exists in the world")]
  UnknownZone(ZoneId),

  #[error("cannot add zone {0}: a zone with that id is already in the world")]
  DuplicateZone(ZoneId),
}

// ── SimWorld ─────────────────────────────────────────────────────────────────

/// The top-level simulated world.  Built once from a property + its
/// zones + the catalog, then stepped forward with
/// [`SimWorld::advance`].
#[derive(Debug)]
pub struct SimWorld {
  pub clock: SimClock,
  pub zones: Vec<Zone>,
  pub catalog: Arc<Catalog>,
  pub weather: WeatherModel,
  /// Per-zone soil state, keyed by zone id.
  pub soil: HashMap<ZoneId, SoilState>,
  /// Per-zone valve state, keyed by zone id.
  pub valves: HashMap<ZoneId, ValveState>,
  /// Flat time-series history of per-sub-step samples.  One entry
  /// per zone per recorded sub-step; see [`SimWorld::advance`] for
  /// the recording cadence.
  pub history: Vec<SensorSample>,
  /// Minutes between recorded history samples.  Smaller = higher
  /// fidelity snapshots; larger = smaller history vectors.  Always
  /// at least one, always a divisor of 1 440 for aesthetics.
  pub record_every_minutes: i64,
}

impl SimWorld {
  /// Build a new simulated world.  The zones slice is expected to
  /// come from a loaded property fixture; the catalog supplies soil
  /// params and emitter specs by id.
  pub fn new(
    start_date: NaiveDate,
    climate_zone: &str,
    zones: Vec<Zone>,
    catalog: Arc<Catalog>,
    seed: u64,
    initial_vwc: f64,
    weather_overrides: Vec<WeatherOverride>,
  ) -> Result<Self, SimWorldError> {
    let climatology = Climatology::for_zone(climate_zone).ok_or_else(|| {
      SimWorldError::UnknownClimateZone(climate_zone.to_string())
    })?;

    // Verify every zone's catalog refs resolve *before* we start
    // stepping, so a typo in a fixture fails loudly rather than
    // silently producing zero moisture changes.
    for z in &zones {
      if !catalog.soil_types.contains_key(&z.soil_type_id) {
        return Err(SimWorldError::UnknownSoilType(
          z.id.clone(),
          z.soil_type_id.clone(),
        ));
      }
      if !catalog.emitters.contains_key(&z.emitter_spec_id) {
        return Err(SimWorldError::UnknownEmitterSpec(
          z.id.clone(),
          z.emitter_spec_id.clone(),
        ));
      }
    }

    let soil: HashMap<ZoneId, SoilState> = zones
      .iter()
      .map(|z| (z.id.clone(), SoilState::new(initial_vwc)))
      .collect();
    let valves: HashMap<ZoneId, ValveState> = zones
      .iter()
      .map(|z| (z.id.clone(), ValveState::default()))
      .collect();

    Ok(Self {
      clock: SimClock::new(start_date),
      zones,
      catalog,
      weather: WeatherModel::new(seed, climatology, weather_overrides),
      soil,
      valves,
      history: Vec::new(),
      record_every_minutes: 60,
    })
  }

  /// Open a zone's valve for the given duration starting now.  A
  /// second open while already open extends the run to the later of
  /// the two end-instants.
  pub fn open_zone(
    &mut self,
    zone_id: &ZoneId,
    duration: SimDuration,
  ) -> Result<(), SimWorldError> {
    let now = self.clock.now();
    let end = SimInstant::from_seconds(
      now.seconds().saturating_add(duration.total_seconds()),
    );
    let state = self
      .valves
      .get_mut(zone_id)
      .ok_or_else(|| SimWorldError::UnknownZone(zone_id.clone()))?;
    state.open_until = Some(match state.open_until {
      Some(existing) if existing > end => existing,
      _ => end,
    });
    Ok(())
  }

  /// Add a zone to the running world.  Validates catalog refs the
  /// same way `new` does so a stale soil-type / emitter-spec id
  /// fails loudly at the call site rather than silently producing
  /// no moisture changes.  Initial soil moisture is the same value
  /// the simulator's boot path uses (default 0.30).
  pub fn add_zone(
    &mut self,
    zone: Zone,
    initial_vwc: f64,
  ) -> Result<(), SimWorldError> {
    if self.zones.iter().any(|z| z.id == zone.id) {
      return Err(SimWorldError::DuplicateZone(zone.id));
    }
    if !self.catalog.soil_types.contains_key(&zone.soil_type_id) {
      return Err(SimWorldError::UnknownSoilType(
        zone.id.clone(),
        zone.soil_type_id.clone(),
      ));
    }
    if !self.catalog.emitters.contains_key(&zone.emitter_spec_id) {
      return Err(SimWorldError::UnknownEmitterSpec(
        zone.id.clone(),
        zone.emitter_spec_id.clone(),
      ));
    }
    self
      .soil
      .insert(zone.id.clone(), SoilState::new(initial_vwc));
    self.valves.insert(zone.id.clone(), ValveState::default());
    self.zones.push(zone);
    Ok(())
  }

  /// Replace the definition of an existing zone in place.  The
  /// zone's id is taken from `updated.id`; the previous zone with
  /// that id is overwritten.  Soil + valve state are preserved
  /// (the new definition shares the same operational history).
  /// Catalog refs are re-validated.
  pub fn update_zone(&mut self, updated: Zone) -> Result<(), SimWorldError> {
    let idx = self
      .zones
      .iter()
      .position(|z| z.id == updated.id)
      .ok_or_else(|| SimWorldError::UnknownZone(updated.id.clone()))?;
    if !self.catalog.soil_types.contains_key(&updated.soil_type_id) {
      return Err(SimWorldError::UnknownSoilType(
        updated.id.clone(),
        updated.soil_type_id.clone(),
      ));
    }
    if !self.catalog.emitters.contains_key(&updated.emitter_spec_id) {
      return Err(SimWorldError::UnknownEmitterSpec(
        updated.id.clone(),
        updated.emitter_spec_id.clone(),
      ));
    }
    self.zones[idx] = updated;
    Ok(())
  }

  /// Remove a zone from the world.  Drops the soil + valve state
  /// for that id and prunes the recorded history.  Returns the
  /// removed `Zone` so callers can confirm what was deleted.
  pub fn remove_zone(
    &mut self,
    zone_id: &ZoneId,
  ) -> Result<Zone, SimWorldError> {
    let idx = self
      .zones
      .iter()
      .position(|z| &z.id == zone_id)
      .ok_or_else(|| SimWorldError::UnknownZone(zone_id.clone()))?;
    let removed = self.zones.remove(idx);
    self.soil.remove(zone_id);
    self.valves.remove(zone_id);
    self.history.retain(|s| &s.zone_id != zone_id);
    Ok(removed)
  }

  pub fn close_zone(&mut self, zone_id: &ZoneId) -> Result<(), SimWorldError> {
    let state = self
      .valves
      .get_mut(zone_id)
      .ok_or_else(|| SimWorldError::UnknownZone(zone_id.clone()))?;
    state.open_until = None;
    Ok(())
  }

  /// Advance the world by `duration`, stepping internally in one-
  /// minute sub-steps.  Records a sample per zone every
  /// `self.record_every_minutes`.  Deterministic given the seed and
  /// inputs.
  pub fn advance(&mut self, duration: SimDuration) {
    let total_minutes = duration.total_minutes().max(0);
    for _ in 0..total_minutes {
      self.step_one_minute();
    }
  }

  fn step_one_minute(&mut self) {
    let now = self.clock.now();
    let dt = self.clock.to_datetime(now);
    let weather = self.weather.sample_at(&self.clock, now);
    let et0 = reference_et0_mm_per_day(self.weather.climatology(), dt);

    // Walk zones in a stable order (their vector order, which comes
    // from the property fixture) so snapshots do not depend on the
    // HashMap iteration order.
    let record_now = now.minutes() % self.record_every_minutes == 0;
    // Clone the zone vector so we can mutate self inside the loop
    // below.  Typical properties carry on the order of ten zones, so
    // the clone is cheap compared to the physics work per sub-step.
    let zones = self.zones.clone();
    for z in &zones {
      self.step_zone(z, &weather, et0, record_now, now);
    }

    // Close any valve whose scheduled end has passed before the next
    // sub-step begins.
    for v in self.valves.values_mut() {
      if let Some(end) = v.open_until {
        if now >= end {
          v.open_until = None;
        }
      }
    }

    self.clock.step(SimDuration::minutes(1));
  }

  fn step_zone(
    &mut self,
    zone: &Zone,
    weather: &WeatherSample,
    et0_mm_per_day: f64,
    record: bool,
    now: SimInstant,
  ) {
    // Look up catalog pieces.  Unwraps are sound because `new` has
    // already checked the refs.
    let soil_type = self
      .catalog
      .soil_types
      .get(&zone.soil_type_id)
      .expect("soil type checked at SimWorld::new");
    let emitter = self
      .catalog
      .emitters
      .get(&zone.emitter_spec_id)
      .expect("emitter spec checked at SimWorld::new");

    // Decide irrigation inflow based on current valve state.  Close
    // happens at the top of the next sub-step; here we count this
    // sub-step as open if the valve is open *at the start* of it.
    let valve = self.valves.get_mut(&zone.id).expect("valve present");
    let valve_is_open = valve.is_open(now);
    let irrigation_mm_per_hour = if valve_is_open {
      zone_inflow_mm_per_hour(zone, emitter)
    } else {
      0.0
    };
    if valve_is_open {
      valve.total_open_seconds += 60;
    }

    let inflow_mm_per_hour =
      irrigation_mm_per_hour + weather.precipitation_mm_per_hour.max(0.0);

    let state = *self.soil.get(&zone.id).expect("soil present");
    let params = SoilParams {
      soil: soil_type,
      plant_kind: zone.plant_kind,
      root_depth_inches: default_root_depth_inches(zone.plant_kind),
    };
    let update =
      soil_step(state, params, inflow_mm_per_hour, et0_mm_per_day, 60);
    self
      .soil
      .insert(zone.id.clone(), SoilState::new(update.new_vwc));

    if record {
      self.history.push(SensorSample {
        instant_minutes: now.minutes(),
        zone_id: zone.id.clone(),
        soil_vwc: update.new_vwc,
        valve_open: valve_is_open,
        weather_temperature_c: weather.temperature_c,
        weather_precipitation_mm_per_hour: weather.precipitation_mm_per_hour,
      });
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::catalog::{EmitterShape, EmitterSpec, SoilType};
  use crate::sim::id::{
    EmitterSpecId, ManifoldInstanceId, SoilTypeId, YardId, ZoneId,
  };
  use crate::sim::zone::PlantKind;

  fn seed_catalog() -> Arc<Catalog> {
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
    Arc::new(cat)
  }

  fn test_zone() -> Zone {
    Zone {
      id: ZoneId::new("z1"),
      yard_id: YardId::new("y"),
      manifold_id: ManifoldInstanceId::new("m"),
      plant_kind: PlantKind::VeggieBed,
      emitter_spec_id: EmitterSpecId::new("inline-drip-12in"),
      soil_type_id: SoilTypeId::new("silty-clay-loam"),
      area_sq_ft: 50.0,
      notes: None,
    }
  }

  #[test]
  fn unknown_climate_zone_rejected() {
    let err = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "mars-olympus",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(err, SimWorldError::UnknownClimateZone(_)));
  }

  #[test]
  fn unknown_soil_type_rejected() {
    let cat = Arc::new(Catalog::default());
    let err = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      cat,
      1,
      0.30,
      Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(err, SimWorldError::UnknownSoilType(_, _)));
  }

  #[test]
  fn advance_records_history() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .expect("world");
    world.advance(SimDuration::hours(4));
    // 4 hours × 60 min / 60 min recording cadence = 4 samples.
    assert_eq!(world.history.len(), 4);
  }

  #[test]
  fn opening_zone_raises_moisture() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.20,
      Vec::new(),
    )
    .expect("world");
    world
      .open_zone(&ZoneId::new("z1"), SimDuration::minutes(15))
      .expect("open");
    world.advance(SimDuration::minutes(30));
    let vwc = world.soil[&ZoneId::new("z1")].vwc;
    assert!(
      vwc > 0.20,
      "expected irrigation to raise moisture, got {vwc} (started 0.20)"
    );
  }

  #[test]
  fn add_zone_inserts_state() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .expect("world");
    let mut new_zone = test_zone();
    new_zone.id = ZoneId::new("z2");
    world.add_zone(new_zone, 0.25).expect("add");
    assert_eq!(world.zones.len(), 2);
    assert_eq!(world.soil[&ZoneId::new("z2")].vwc, 0.25);
    assert!(world.valves.contains_key(&ZoneId::new("z2")));
  }

  #[test]
  fn add_zone_rejects_duplicate_id() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .expect("world");
    let err = world.add_zone(test_zone(), 0.30).unwrap_err();
    assert!(matches!(err, SimWorldError::DuplicateZone(_)));
  }

  #[test]
  fn add_zone_rejects_unknown_emitter() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .expect("world");
    let mut bad = test_zone();
    bad.id = ZoneId::new("z2");
    bad.emitter_spec_id = EmitterSpecId::new("nonexistent");
    let err = world.add_zone(bad, 0.30).unwrap_err();
    assert!(matches!(err, SimWorldError::UnknownEmitterSpec(_, _)));
  }

  #[test]
  fn update_zone_replaces_definition_keeps_state() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .expect("world");
    let original_vwc = world.soil[&ZoneId::new("z1")].vwc;
    let mut updated = test_zone();
    updated.area_sq_ft = 999.0;
    updated.notes = Some("new notes".into());
    world.update_zone(updated).expect("update");
    assert_eq!(world.zones[0].area_sq_ft, 999.0);
    // Soil state preserved.
    assert_eq!(world.soil[&ZoneId::new("z1")].vwc, original_vwc);
  }

  #[test]
  fn update_zone_rejects_unknown_id() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .expect("world");
    let mut bogus = test_zone();
    bogus.id = ZoneId::new("ghost");
    let err = world.update_zone(bogus).unwrap_err();
    assert!(matches!(err, SimWorldError::UnknownZone(_)));
  }

  #[test]
  fn remove_zone_drops_state_and_history() {
    let mut world = SimWorld::new(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      "portland-or",
      vec![test_zone()],
      seed_catalog(),
      1,
      0.30,
      Vec::new(),
    )
    .expect("world");
    world.advance(SimDuration::hours(2));
    assert!(!world.history.is_empty());
    let removed = world.remove_zone(&ZoneId::new("z1")).expect("remove");
    assert_eq!(removed.id.as_str(), "z1");
    assert!(world.zones.is_empty());
    assert!(world.soil.is_empty());
    assert!(world.valves.is_empty());
    assert!(world.history.is_empty());
  }

  #[test]
  fn two_runs_with_same_seed_match() {
    let mk = || {
      SimWorld::new(
        NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        "portland-or",
        vec![test_zone()],
        seed_catalog(),
        42,
        0.30,
        Vec::new(),
      )
      .unwrap()
    };
    let mut a = mk();
    let mut b = mk();
    a.advance(SimDuration::days(3));
    b.advance(SimDuration::days(3));
    assert_eq!(a.history, b.history);
  }
}
