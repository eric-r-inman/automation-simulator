//! Snapshot of the seeded `data/catalog/` row counts.
//!
//! Acts as a regression net: a PR that adds or removes catalog
//! rows updates the snapshot under review (`cargo insta review`),
//! so silent drift in the dropdowns or the planner's option set
//! is impossible.

use std::collections::BTreeMap;
use std::path::PathBuf;

use automation_simulator_lib::catalog::Catalog;

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
}

#[test]
fn seeded_catalog_counts_match_snapshot() {
  let cat = Catalog::load(workspace_root().join("data").join("catalog"))
    .expect("seeded catalog loads");
  let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
  counts.insert("controllers", cat.controllers.len());
  counts.insert("sensors", cat.sensors.len());
  counts.insert("weather_stations", cat.weather_stations.len());
  counts.insert("manifolds", cat.manifolds.len());
  counts.insert("valves", cat.valves.len());
  counts.insert("emitters", cat.emitters.len());
  counts.insert("pressure_regulators", cat.pressure_regulators.len());
  counts.insert("backflow_preventers", cat.backflow_preventers.len());
  counts.insert("drip_lines", cat.drip_lines.len());
  counts.insert("compute_hosts", cat.compute_hosts.len());
  counts.insert("species", cat.species.len());
  counts.insert("soil_types", cat.soil_types.len());
  insta::assert_yaml_snapshot!(counts);
}

#[test]
fn seeded_catalog_controller_ids_match_snapshot() {
  let cat = Catalog::load(workspace_root().join("data").join("catalog"))
    .expect("catalog");
  let mut ids: Vec<String> = cat
    .controllers
    .keys()
    .map(|k| k.as_str().to_string())
    .collect();
  ids.sort();
  insta::assert_yaml_snapshot!(ids);
}

#[test]
fn seeded_catalog_emitter_ids_match_snapshot() {
  let cat = Catalog::load(workspace_root().join("data").join("catalog"))
    .expect("catalog");
  let mut ids: Vec<String> = cat
    .emitters
    .keys()
    .map(|k| k.as_str().to_string())
    .collect();
  ids.sort();
  insta::assert_yaml_snapshot!(ids);
}
