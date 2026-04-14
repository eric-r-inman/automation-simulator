//! Catalog loader tests.
//!
//! Each test builds a tiny catalog directory in a `tempfile::TempDir`
//! rather than writing to the real `data/catalog/`; this way changes
//! to the seeded fixtures never break the loader tests, and the
//! loader tests never break the seeded fixtures.

use std::fs;
use std::path::PathBuf;

use super::*;

fn write(dir: &Path, filename: &str, contents: &str) {
  fs::write(dir.join(filename), contents).expect("write catalog file");
}

#[test]
fn empty_directory_loads_to_empty_catalog() {
  let tmp = tempfile::TempDir::new().expect("tempdir");
  let cat = Catalog::load(tmp.path()).expect("empty catalog loads");
  assert!(cat.controllers.is_empty());
  assert!(cat.species.is_empty());
}

#[test]
fn well_formed_single_category_loads() {
  let tmp = tempfile::TempDir::new().expect("tempdir");
  write(
    tmp.path(),
    "controllers.toml",
    r#"
[example-24v-controller]
id = "example-24v-controller"
name = "Example 24 VAC Controller"
manufacturer = "Example Manufacturer"
price_usd = 90.0
max_zones = 6
valve_voltage_ac = 24.0
"#,
  );

  let cat = Catalog::load(tmp.path()).expect("catalog loads");
  let id = crate::sim::id::ControllerModelId::new("example-24v-controller");
  let c = cat.controllers.get(&id).expect("controller present");
  assert_eq!(c.max_zones, 6);
}

#[test]
fn key_and_id_must_match() {
  let tmp = tempfile::TempDir::new().expect("tempdir");
  write(
    tmp.path(),
    "controllers.toml",
    r#"
[intended-key]
id = "actual-id-mismatch"
name = "oops"
manufacturer = "oops"
price_usd = 1.0
max_zones = 1
valve_voltage_ac = 24.0
"#,
  );

  let err = Catalog::load(tmp.path()).unwrap_err();
  assert!(matches!(err, CatalogLoadError::KeyIdMismatch { .. }));
}

#[test]
fn unknown_sensor_gateway_rejected() {
  let tmp = tempfile::TempDir::new().expect("tempdir");
  write(
    tmp.path(),
    "sensors.toml",
    r#"
[soil-a]
id = "soil-a"
name = "Soil Sensor A"
manufacturer = "Example"
price_usd = 20.0
kind = "soil-moisture"
gateway_model_id = "ghost-gateway"
"#,
  );

  let err = Catalog::load(tmp.path()).unwrap_err();
  assert!(matches!(err, CatalogLoadError::UnknownSensorGateway { .. }));
}

#[test]
fn sensor_gateway_resolves_when_present() {
  let tmp = tempfile::TempDir::new().expect("tempdir");
  write(
    tmp.path(),
    "sensors.toml",
    r#"
[gateway-a]
id = "gateway-a"
name = "Gateway A"
manufacturer = "Example"
price_usd = 50.0
kind = "gateway"

[soil-a]
id = "soil-a"
name = "Soil Sensor A"
manufacturer = "Example"
price_usd = 20.0
kind = "soil-moisture"
gateway_model_id = "gateway-a"
"#,
  );

  let cat = Catalog::load(tmp.path()).expect("loads");
  assert_eq!(cat.sensors.len(), 2);
}

#[test]
fn non_monotone_soil_rejected() {
  let tmp = tempfile::TempDir::new().expect("tempdir");
  write(
    tmp.path(),
    "soil-types.toml",
    r#"
[bad-soil]
id = "bad-soil"
name = "Bad Soil"
saturation_vwc = 0.30
field_capacity_vwc = 0.35
wilting_point_vwc = 0.15
saturated_hydraulic_conductivity_mm_per_hr = 20.0
"#,
  );

  let err = Catalog::load(tmp.path()).unwrap_err();
  assert!(matches!(err, CatalogLoadError::NonMonotoneSoilVwc(_)));
}

#[test]
fn inline_emitter_without_spacing_rejected() {
  let tmp = tempfile::TempDir::new().expect("tempdir");
  write(
    tmp.path(),
    "emitters.toml",
    r#"
[inline-no-spacing]
id = "inline-no-spacing"
name = "Inline without spacing"
manufacturer = "Example"
price_usd_per_100 = 30.0
shape = "inline-drip"
flow_gph = 0.9
min_inlet_psi = 15.0
pressure_compensating = true
"#,
  );

  let err = Catalog::load(tmp.path()).unwrap_err();
  assert!(matches!(err, CatalogLoadError::InlineEmitterMissingSpacing { .. }));
}

#[test]
fn seeded_catalog_loads_and_resolves() {
  // This test points at the real data/catalog/ so a broken seed
  // file fails the build rather than the Phase 5 loader later.
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf();
  let dir = root.join("data").join("catalog");
  if !dir.exists() {
    // Seed data is written in a later step of Phase 2.5; tolerate
    // the directory being absent during the initial compile-check.
    return;
  }
  Catalog::load(&dir).expect("seeded catalog loads");
}
