//! 20-day demo scenario regression test.
//!
//! Runs the seeded `data/scenarios/2026-07-heatwave.toml` against
//! the example property end-to-end through `SimWorld` — no HTTP,
//! no database — and asserts:
//!
//! 1. The validated scenario carries 20 manual interventions and a
//!    20-day duration.
//! 2. After replaying every intervention and stepping through the
//!    full duration, every recorded soil-moisture sample stays
//!    inside `[0.10, 0.48]` (the saturation cap, with a small
//!    drought tolerance for the heatwave-stressed window).
//! 3. The watering log on Zone A1 matches the scenario: 20 runs
//!    × 15 minutes = 300 simulated minutes of valve open time.
//! 4. The run is byte-deterministic across two passes given the
//!    same seed.

use std::sync::Arc;

use chrono::NaiveDate;

use automation_simulator_lib::{
  catalog::Catalog,
  engine::{SimDuration, SimWorld},
  seed::load_property,
  sim::id::ZoneId,
  sim::scenario::{ManualIntervention, Scenario, ScenarioRaw},
};

fn workspace_root() -> std::path::PathBuf {
  std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
}

fn load_scenario() -> Scenario {
  let path = workspace_root()
    .join("data")
    .join("scenarios")
    .join("2026-07-heatwave.toml");
  let text = std::fs::read_to_string(&path).expect("read scenario");
  let raw: ScenarioRaw = toml::from_str(&text).expect("parse scenario");
  Scenario::try_from_raw(raw).expect("validate scenario")
}

fn build_world() -> SimWorld {
  let catalog = Arc::new(
    Catalog::load(workspace_root().join("data").join("catalog"))
      .expect("catalog"),
  );
  let bundle = load_property(
    workspace_root()
      .join("data")
      .join("properties")
      .join("example-property.toml"),
    &catalog,
  )
  .expect("bundle");
  SimWorld::new(
    NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
    &bundle.property.climate_zone,
    bundle.zones.clone(),
    Arc::clone(&catalog),
    42,
    0.30,
    Vec::new(),
  )
  .expect("sim world")
}

/// Replay a validated scenario against a freshly built world.
/// Sorts manual interventions by `offset_minutes` so the engine
/// always sees them in the right order regardless of TOML order.
fn replay(world: &mut SimWorld, scenario: &Scenario) {
  let mut events: Vec<&ManualIntervention> =
    scenario.manual_interventions.iter().collect();
  events.sort_by_key(|m| match m {
    ManualIntervention::RunZone { offset_minutes, .. }
    | ManualIntervention::StopZone { offset_minutes, .. } => *offset_minutes,
  });

  let mut cursor: i64 = 0;
  for ev in &events {
    let (offset, _zone_id) = match ev {
      ManualIntervention::RunZone {
        offset_minutes,
        zone_id,
        ..
      } => (*offset_minutes, zone_id.clone()),
      ManualIntervention::StopZone {
        offset_minutes,
        zone_id,
        ..
      } => (*offset_minutes, zone_id.clone()),
    };
    if offset > cursor {
      world.advance(SimDuration::minutes(offset - cursor));
      cursor = offset;
    }
    match ev {
      ManualIntervention::RunZone {
        zone_id,
        duration_minutes,
        ..
      } => {
        world
          .open_zone(zone_id, SimDuration::minutes(*duration_minutes))
          .expect("open");
      }
      ManualIntervention::StopZone { zone_id, .. } => {
        world.close_zone(zone_id).expect("close");
      }
    }
  }

  // Run out the rest of the scenario's duration so the recorded
  // history covers the full window.
  if scenario.duration_minutes > cursor {
    world.advance(SimDuration::minutes(scenario.duration_minutes - cursor));
  }
}

#[test]
fn scenario_validates_with_expected_shape() {
  let scenario = load_scenario();
  assert_eq!(scenario.duration_minutes, 28800);
  assert_eq!(scenario.rng_seed, 42);
  assert_eq!(scenario.manual_interventions.len(), 20);
  assert_eq!(scenario.weather_overrides.len(), 1);
}

#[test]
fn scenario_validates_against_property_zones() {
  let scenario = load_scenario();
  // Cross-check against the example property's zones — the
  // scenario must only reference zones that exist in the fixture.
  let catalog = Arc::new(
    Catalog::load(workspace_root().join("data").join("catalog"))
      .expect("catalog"),
  );
  let bundle = load_property(
    workspace_root()
      .join("data")
      .join("properties")
      .join("example-property.toml"),
    &catalog,
  )
  .expect("bundle");
  let known_ids: Vec<ZoneId> =
    bundle.zones.iter().map(|z| z.id.clone()).collect();
  scenario
    .validate_against(known_ids.iter())
    .expect("scenario references only known zones");
}

#[test]
fn replay_produces_plausible_moisture_history() {
  let scenario = load_scenario();
  let mut world = build_world();
  replay(&mut world, &scenario);

  // Every recorded sample stays within a defensible plant-stress
  // window.  Lower bound is loose because the engineered three-day
  // heatwave can push the smallest zones near wilting briefly;
  // upper bound is the soil's saturation.
  for sample in &world.history {
    assert!(
      (0.10..=0.48).contains(&sample.soil_vwc),
      "sample at minute {} for {} = {} fell outside [0.10, 0.48]",
      sample.instant_minutes,
      sample.zone_id,
      sample.soil_vwc
    );
  }

  // Zone A1 (the one the scenario waters daily) must accumulate
  // 20 runs × 15 minutes = 300 minutes = 18 000 seconds of valve
  // open time.
  let zone_a1 = ZoneId::new("zone-a1-veggies");
  let total = world
    .valves
    .get(&zone_a1)
    .expect("valve present")
    .total_open_seconds;
  assert_eq!(
    total, 18_000,
    "expected 18 000 seconds of open time on {} after 20 daily 15-min runs, \
     got {}",
    zone_a1, total
  );

  // The scenario records hourly samples, so 20 days * 24 h = 480
  // samples per zone × 6 zones = 2 880 expected entries.
  assert_eq!(
    world.history.len(),
    480 * 6,
    "expected 480 samples per zone over 20 days × 6 zones"
  );
}

#[test]
fn replay_is_deterministic() {
  let scenario = load_scenario();
  let mut a = build_world();
  let mut b = build_world();
  replay(&mut a, &scenario);
  replay(&mut b, &scenario);
  assert_eq!(a.history, b.history);
}
