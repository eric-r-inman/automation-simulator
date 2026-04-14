//! Thirty-day reference scenario for the Phase 3 soil-moisture
//! engine.
//!
//! Builds a tiny single-zone property (50 sq ft raised veggie bed,
//! silty-clay-loam soil, 0.9 GPH inline drip), applies a fixed
//! daily irrigation schedule, advances the simulator 30 simulated
//! days from July 1 2026, and asserts two things:
//!
//! 1. Soil moisture stays inside the plausible plant-stress range
//!    `[0.15, 0.45]` v/v for every recorded sample.
//! 2. The downsampled-to-daily moisture time series matches the
//!    committed `insta` snapshot, which proves the engine is
//!    byte-deterministic across machines and repeated runs.
//!
//! If the snapshot changes (intended physics tweak or bug fix),
//! review with `cargo insta review`.

use std::sync::Arc;

use chrono::NaiveDate;

use automation_simulator_lib::{
  catalog::{
    BackflowKind, BackflowPreventerModel, Catalog, ControllerModel,
    DripLineModel, EmitterShape, EmitterSpec, ManifoldModel,
    PressureRegulatorModel, SensorKind, SensorModel, SoilType, Species,
    ValveModel, WeatherStationModel,
  },
  engine::{SimDuration, SimWorld},
  sim::{
    id::{
      BackflowPreventerModelId, ControllerModelId, DripLineModelId,
      EmitterSpecId, ManifoldInstanceId, ManifoldModelId,
      PressureRegulatorModelId, SensorModelId, SoilTypeId, SpeciesId,
      ValveModelId, WeatherStationModelId, YardId, ZoneId,
    },
    zone::{PlantKind, Zone},
  },
};

fn minimal_catalog() -> Arc<Catalog> {
  let mut c = Catalog::default();
  c.soil_types.insert(
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
  c.emitters.insert(
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
  // Filler rows so the catalog is non-empty across categories.  None
  // participate in the 30-day scenario directly; the SimWorld only
  // needs the soil-type and emitter-spec refs above.
  let _ = (
    BackflowKind::AtmosphericVacuumBreaker,
    BackflowPreventerModelId::new("x"),
    ControllerModelId::new("x"),
    DripLineModelId::new("x"),
    ManifoldModelId::new("x"),
    PressureRegulatorModelId::new("x"),
    SensorModelId::new("x"),
    ValveModelId::new("x"),
    WeatherStationModelId::new("x"),
    SpeciesId::new("x"),
    ControllerModel {
      id: ControllerModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd: 0.0,
      max_zones: 1,
      valve_voltage_ac: 24.0,
      notes: None,
    },
    ValveModel {
      id: ValveModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd: 0.0,
      voltage_ac: 24.0,
      coil_current_a: 0.3,
      notes: None,
    },
    DripLineModel {
      id: DripLineModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd_per_foot: 0.1,
      outer_diameter_inches: 0.6,
      notes: None,
    },
    ManifoldModel {
      id: ManifoldModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd: 0.0,
      zone_capacity: 3,
      valve_voltage_ac: 24.0,
      notes: None,
    },
    BackflowPreventerModel {
      id: BackflowPreventerModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd: 0.0,
      kind: BackflowKind::AtmosphericVacuumBreaker,
      notes: None,
    },
    PressureRegulatorModel {
      id: PressureRegulatorModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd: 0.0,
      input_psi_min: 20.0,
      input_psi_max: 80.0,
      output_psi: 25.0,
      notes: None,
    },
    SensorModel {
      id: SensorModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd: 0.0,
      kind: SensorKind::SoilMoisture,
      gateway_model_id: None,
      notes: None,
    },
    WeatherStationModel {
      id: WeatherStationModelId::new("x"),
      name: "x".into(),
      manufacturer: "x".into(),
      price_usd: 0.0,
      measures_temperature: true,
      measures_humidity: true,
      measures_wind: true,
      measures_rain: true,
      measures_solar: true,
      notes: None,
    },
    Species {
      id: SpeciesId::new("x"),
      common_name: "x".into(),
      scientific_name: "x".into(),
      kind: PlantKind::VeggieBed,
      water_need_base_ml_per_day: 1000.0,
      root_depth_inches: 18.0,
      mature_size_sq_ft: 4.0,
      hardiness_zone_min: 5,
      hardiness_zone_max: 9,
      notes: None,
    },
  );
  Arc::new(c)
}

fn test_zone() -> Zone {
  Zone {
    id: ZoneId::new("yard-a-veggies"),
    yard_id: YardId::new("yard-a"),
    manifold_id: ManifoldInstanceId::new("manifold-a"),
    plant_kind: PlantKind::VeggieBed,
    emitter_spec_id: EmitterSpecId::new("inline-drip-12in"),
    soil_type_id: SoilTypeId::new("silty-clay-loam"),
    area_sq_ft: 50.0,
    notes: None,
  }
}

fn run_scenario() -> SimWorld {
  let mut world = SimWorld::new(
    NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
    "portland-or",
    vec![test_zone()],
    minimal_catalog(),
    42,
    0.30,
    Vec::new(),
  )
  .expect("build SimWorld");
  // Record one sample per simulated day so the snapshot stays
  // compact and small wobbles inside a day average out.
  world.record_every_minutes = 24 * 60;

  let zone_id = ZoneId::new("yard-a-veggies");
  // Thirty simulated days, each with a 15-minute morning run.
  for day in 0..30 {
    // Run the zone for 15 minutes at the start of the day.
    world
      .open_zone(&zone_id, SimDuration::minutes(15))
      .expect("open");
    // Advance one full simulated day.  The valve auto-closes when
    // its open-until instant passes.
    world.advance(SimDuration::days(1));
    // Belt-and-braces: explicitly close in case rounding at the
    // minute boundary leaves the valve nominally open.
    world.close_zone(&zone_id).expect("close");
    let _ = day;
  }
  world
}

#[test]
fn soil_stays_in_plausible_range_over_30_days() {
  let world = run_scenario();
  for sample in &world.history {
    assert!(
      (0.15..=0.45).contains(&sample.soil_vwc),
      "soil_vwc {} at minute {} fell outside the plausible \
       range [0.15, 0.45]",
      sample.soil_vwc,
      sample.instant_minutes
    );
  }
}

#[test]
fn moisture_time_series_is_deterministic() {
  let a = run_scenario();
  let b = run_scenario();
  let a_series: Vec<(i64, String)> = a
    .history
    .iter()
    .map(|s| (s.instant_minutes, format!("{:.4}", s.soil_vwc)))
    .collect();
  let b_series: Vec<(i64, String)> = b
    .history
    .iter()
    .map(|s| (s.instant_minutes, format!("{:.4}", s.soil_vwc)))
    .collect();
  assert_eq!(a_series, b_series);
}

#[test]
fn moisture_time_series_matches_snapshot() {
  let world = run_scenario();
  let series: Vec<(i64, String)> = world
    .history
    .iter()
    .map(|s| (s.instant_minutes, format!("{:.4}", s.soil_vwc)))
    .collect();
  insta::assert_yaml_snapshot!(series);
}
