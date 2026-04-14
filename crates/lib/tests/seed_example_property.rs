//! End-to-end seed test.
//!
//! Loads the example property fixture against the real
//! `data/catalog/` tree, seeds it into an in-memory SQLite database,
//! and verifies the row counts match the fixture's declarations.  A
//! malformed fixture test confirms the loader names the offending
//! field.

use std::path::PathBuf;

use automation_simulator_lib::{
  catalog::Catalog,
  db::SimDb,
  seed::{load_property, seed_property, SeedError},
};

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
}

fn catalog() -> Catalog {
  Catalog::load(workspace_root().join("data").join("catalog"))
    .expect("catalog loads")
}

fn fixture_path(name: &str) -> PathBuf {
  workspace_root().join("data").join("properties").join(name)
}

#[tokio::test]
async fn example_property_seeds_into_memory_db() {
  let db = SimDb::connect(":memory:").await.expect("open");
  let bundle =
    seed_property(fixture_path("example-property.toml"), &catalog(), &db)
      .await
      .expect("seed");

  // Counts from the fixture: 2 yards, 2 spigots, 2 manifolds, 6
  // zones, 10 plants, 1 controller, 6 sensors, 1 weather station,
  // 6 schedule entries.
  assert_eq!(bundle.property.yards.len(), 2);
  assert_eq!(bundle.property.spigots.len(), 2);
  assert_eq!(bundle.manifolds.len(), 2);
  assert_eq!(bundle.zones.len(), 6);
  assert_eq!(bundle.plants.len(), 10);
  assert_eq!(bundle.controllers.len(), 1);
  assert_eq!(bundle.sensors.len(), 6);
  assert_eq!(bundle.weather_stations.len(), 1);
  assert_eq!(bundle.schedule.len(), 6);

  // Read the persisted zones back and confirm the shape matches.
  let zones = db
    .zones_for_property(bundle.property.id.as_str())
    .await
    .expect("zones_for_property");
  assert_eq!(zones.len(), 6);

  let veggie_zones = zones
    .iter()
    .filter(|z| z.plant_kind == "veggie-bed")
    .count();
  let shrub_zones = zones.iter().filter(|z| z.plant_kind == "shrub").count();
  let perennial_zones =
    zones.iter().filter(|z| z.plant_kind == "perennial").count();
  assert_eq!(veggie_zones, 2);
  assert_eq!(shrub_zones, 2);
  assert_eq!(perennial_zones, 2);
}

#[tokio::test]
async fn validate_without_persisting_also_works() {
  // Calling load_property without touching the DB is a useful
  // standalone lint the Phase 5 plan calls out.
  let bundle = load_property(fixture_path("example-property.toml"), &catalog())
    .expect("validate");
  assert_eq!(bundle.property.id.as_str(), "example-property");
}

#[test]
fn bad_catalog_ref_reports_the_offending_id() {
  let cat = catalog();
  // Build a minimal broken fixture pointing at an unknown species.
  let bad_toml = r#"
[property]
id = "broken"
name = "Broken"
climate_zone = "portland-or"
lot_area_sq_ft = 1000.0

[[property.yards]]
id = "y"
name = "Y"
area_sq_ft = 100.0

[[property.spigots]]
id = "s"
mains_pressure_psi = 60.0

[[manifolds]]
id = "m"
model_id = "generic-3zone-manifold"
spigot_id = "s"
zone_capacity = 1

[[zones]]
id = "z"
yard_id = "y"
manifold_id = "m"
plant_kind = "veggie-bed"
emitter_spec_id = "inline-drip-12in"
soil_type_id = "silty-clay-loam"
area_sq_ft = 10.0

[[plants]]
id = "ghost"
zone_id = "z"
species_id = "nonexistent-species"
planted_on = "2026-01-01"
water_need_ml_per_day = 100.0
"#;

  let raw: automation_simulator_lib::seed::PropertyFileRaw =
    toml::from_str(bad_toml).expect("parse");
  let err =
    automation_simulator_lib::seed::PropertyBundle::try_from_raw(raw, &cat)
      .unwrap_err();
  match err {
    SeedError::UnknownSpeciesRef { plant, species } => {
      assert_eq!(plant.as_str(), "ghost");
      assert_eq!(species.as_str(), "nonexistent-species");
    }
    other => panic!("expected UnknownSpeciesRef, got {other:?}"),
  }
}
