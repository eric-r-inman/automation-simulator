//! Property-based tests for the soil-moisture model.
//!
//! Generates a thousand random schedules inside the constraints the
//! domain model would produce, runs each against the reference
//! soil + emitter, and asserts the step never produces NaN, never
//! goes negative, and never exceeds saturation.  This guards the
//! invariants the `SimWorld::advance` loop relies on — if a bug
//! ever lets NaN into the state, a scenario eight days in starts
//! to misbehave silently; the property test catches it at a single
//! step.

use automation_simulator_lib::{
  catalog::SoilType,
  engine::{soil_step, SoilParams, SoilState},
  sim::{id::SoilTypeId, zone::PlantKind},
};
use proptest::prelude::*;

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

proptest! {
  #![proptest_config(ProptestConfig::with_cases(1_000))]

  #[test]
  fn soil_step_never_produces_nan_or_out_of_bounds(
    initial_vwc in 0.0_f64..0.48,
    inflow_mm_per_hour in 0.0_f64..200.0,
    et0_mm_per_day in 0.0_f64..12.0,
    plant_kind_idx in 0usize..4,
  ) {
    let soil = silty_clay_loam();
    let plant_kind = match plant_kind_idx {
      0 => PlantKind::VeggieBed,
      1 => PlantKind::Shrub,
      2 => PlantKind::Perennial,
      _ => PlantKind::Tree,
    };
    let params = SoilParams {
      soil: &soil,
      plant_kind,
      root_depth_inches: 18.0,
    };
    let update = soil_step(
      SoilState::new(initial_vwc),
      params,
      inflow_mm_per_hour,
      et0_mm_per_day,
      60,
    );

    prop_assert!(
      update.new_vwc.is_finite(),
      "new_vwc produced NaN or infinity: {}",
      update.new_vwc
    );
    prop_assert!(
      (0.0..=soil.saturation_vwc + 1e-9).contains(&update.new_vwc),
      "new_vwc {} escaped [0, saturation={}]",
      update.new_vwc,
      soil.saturation_vwc
    );
  }
}
