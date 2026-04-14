//! Per-zone soil-moisture model.
//!
//! State is the volumetric water content `vwc` inside the root zone,
//! dimensionless in `[0, saturation]`.  Each sub-step the engine
//! calls [`soil_step`] with the inflow from irrigation + rain, the
//! evapotranspiration demand from the weather + plant kind, and the
//! sub-step duration.  The step computes drainage internally from
//! the soil parameters.
//!
//! The balance equation is
//!
//! ```text
//! d(vwc) / dt = (I - ET - D) / root_depth_mm
//! ```
//!
//! where
//!
//! - I is inflow in mm/hour, summed from rain + irrigation,
//! - ET is the plant-adjusted evapotranspiration in mm/hour,
//! - D is drainage in mm/hour, nonzero only when vwc exceeds field
//!   capacity and otherwise proportional to saturated hydraulic
//!   conductivity and the excess above field capacity.
//!
//! Two invariants are enforced after every step: vwc stays in
//! `[0, saturation]` and no value ever becomes NaN.  These hold
//! regardless of input — the proptest in `tests/` feeds a random
//! 1 000-schedule sample and asserts the same.

use crate::catalog::SoilType;
use crate::sim::zone::PlantKind;

/// Crop coefficient (Kc) the engine uses when translating reference
/// evapotranspiration (ET0) into an actual ET demand for a zone.
/// Values chosen to stay within the FAO-56 midseason range for
/// temperate food-forest plantings without pretending to be
/// authoritative for any specific species.
pub fn crop_coefficient(kind: PlantKind) -> f64 {
  match kind {
    PlantKind::VeggieBed => 0.90,
    PlantKind::Shrub => 0.60,
    PlantKind::Perennial => 0.55,
    PlantKind::Tree => 0.70,
  }
}

/// Default root-zone depth per plant kind, inches.  Used when the
/// domain model does not carry an explicit per-zone override.
pub fn default_root_depth_inches(kind: PlantKind) -> f64 {
  match kind {
    PlantKind::VeggieBed => 18.0,
    PlantKind::Shrub => 24.0,
    PlantKind::Perennial => 18.0,
    PlantKind::Tree => 36.0,
  }
}

/// Immutable inputs to one soil-moisture step.  Packaged so the step
/// function has a small, typed signature rather than a dozen
/// positional parameters.
#[derive(Debug, Clone, Copy)]
pub struct SoilParams<'a> {
  pub soil: &'a SoilType,
  pub plant_kind: PlantKind,
  pub root_depth_inches: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoilState {
  pub vwc: f64,
}

impl SoilState {
  pub fn new(initial_vwc: f64) -> Self {
    Self { vwc: initial_vwc }
  }
}

/// Summary of a single step's net fluxes, in mm at sub-step
/// resolution.  Returned alongside the updated `vwc` so callers can
/// log the balance and reproduce anomalies from snapshots alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoilUpdate {
  pub inflow_mm: f64,
  pub et_mm: f64,
  pub drainage_mm: f64,
  pub new_vwc: f64,
}

/// Advance a zone's soil moisture by `dt_seconds`.  Inputs:
///
/// - `state`       current volumetric water content
/// - `params`      catalog soil type + plant kind + root depth
/// - `inflow_mm_per_hour`  sum of rain + irrigation flux
/// - `et0_mm_per_day`      reference ET at the zone's location and month
/// - `dt_seconds`          sub-step length — the engine uses 60 s
pub fn soil_step(
  state: SoilState,
  params: SoilParams<'_>,
  inflow_mm_per_hour: f64,
  et0_mm_per_day: f64,
  dt_seconds: i64,
) -> SoilUpdate {
  let dt_hours = dt_seconds as f64 / 3600.0;
  let root_depth_mm = params.root_depth_inches * 25.4;

  let inflow_mm = inflow_mm_per_hour.max(0.0) * dt_hours;

  let kc = crop_coefficient(params.plant_kind);
  let et_mm_per_hour = (et0_mm_per_day * kc) / 24.0;
  let et_mm = et_mm_per_hour.max(0.0) * dt_hours;

  let fc = params.soil.field_capacity_vwc;
  let sat = params.soil.saturation_vwc;
  let ksat = params.soil.saturated_hydraulic_conductivity_mm_per_hr;
  let drainage_mm = if state.vwc > fc {
    // Linear in excess above FC, capped at what is actually stored
    // above FC so drainage never drives vwc below FC in one step.
    let excess_vwc = (state.vwc - fc).max(0.0);
    let max_drainable_mm = excess_vwc * root_depth_mm;
    ((state.vwc - fc) / (sat - fc).max(1e-6) * ksat * dt_hours)
      .max(0.0)
      .min(max_drainable_mm)
  } else {
    0.0
  };

  let net_mm = inflow_mm - et_mm - drainage_mm;
  let delta_vwc = net_mm / root_depth_mm.max(1e-3);
  // Clamp to [0, saturation].  Root-depth clamp plus the drainage
  // cap above keep overshoots small, but a defensive clamp here
  // catches pathological inputs (a one-minute inflow larger than the
  // zone can hold).
  let new_vwc = (state.vwc + delta_vwc).clamp(0.0, sat);
  // NaN propagates through arithmetic; replace with the prior value
  // rather than blowing up.  The proptest asserts this never fires
  // in practice.
  let new_vwc = if new_vwc.is_finite() {
    new_vwc
  } else {
    state.vwc
  };

  SoilUpdate {
    inflow_mm,
    et_mm,
    drainage_mm,
    new_vwc,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::catalog::SoilType;
  use crate::sim::id::SoilTypeId;

  fn silty_clay_loam() -> SoilType {
    SoilType {
      id: SoilTypeId::new("silty-clay-loam"),
      name: "Silty Clay Loam".into(),
      saturation_vwc: 0.48,
      field_capacity_vwc: 0.36,
      wilting_point_vwc: 0.17,
      saturated_hydraulic_conductivity_mm_per_hr: 3.5,
      notes: None,
    }
  }

  fn params<'a>(soil: &'a SoilType) -> SoilParams<'a> {
    SoilParams {
      soil,
      plant_kind: PlantKind::VeggieBed,
      root_depth_inches: 18.0,
    }
  }

  #[test]
  fn zero_inputs_preserve_vwc() {
    let soil = silty_clay_loam();
    let state = SoilState::new(0.30);
    let update = soil_step(state, params(&soil), 0.0, 0.0, 60);
    assert!((update.new_vwc - 0.30).abs() < 1e-9);
  }

  #[test]
  fn et_reduces_vwc_only() {
    let soil = silty_clay_loam();
    let state = SoilState::new(0.30);
    let update = soil_step(state, params(&soil), 0.0, 5.0, 60);
    assert!(update.new_vwc < state.vwc);
    assert_eq!(update.inflow_mm, 0.0);
    assert!(update.et_mm > 0.0);
  }

  #[test]
  fn inflow_raises_vwc() {
    let soil = silty_clay_loam();
    let state = SoilState::new(0.20);
    let update = soil_step(state, params(&soil), 40.0, 4.0, 60);
    assert!(update.new_vwc > state.vwc);
  }

  #[test]
  fn vwc_never_exceeds_saturation() {
    let soil = silty_clay_loam();
    let state = SoilState::new(0.47);
    // Huge inflow; result must still clamp to saturation.
    let update = soil_step(state, params(&soil), 10_000.0, 0.0, 60);
    assert!(update.new_vwc <= soil.saturation_vwc + 1e-9);
  }

  #[test]
  fn vwc_never_goes_negative() {
    let soil = silty_clay_loam();
    let state = SoilState::new(0.05);
    // Huge ET demand; result must clamp at zero.
    let update = soil_step(state, params(&soil), 0.0, 500.0, 60);
    assert!(update.new_vwc >= 0.0);
  }

  #[test]
  fn drainage_only_fires_above_field_capacity() {
    let soil = silty_clay_loam();
    let state_below = SoilState::new(0.30);
    let below = soil_step(state_below, params(&soil), 0.0, 0.0, 60);
    assert_eq!(below.drainage_mm, 0.0);
    let state_above = SoilState::new(0.40);
    let above = soil_step(state_above, params(&soil), 0.0, 0.0, 60);
    assert!(above.drainage_mm > 0.0);
  }
}
