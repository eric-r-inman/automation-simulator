//! Persistence-layer round-trip.
//!
//! Opens an in-memory SQLite database, applies the Phase 4
//! migrations, inserts at least one row per table in dependency
//! order, then reads enough of the graph back to prove the
//! foreign-key wiring is correct.  Any migration drift or
//! serialization mismatch produces a test failure at the exact
//! insert/select pair that broke.

use automation_simulator_lib::db::{
  ControllerInstanceRow, ManifoldRow, PlantRow, PropertyDesignRow, PropertyRow,
  ScheduleItemRow, SensorInstanceRow, SensorReadingRow, SimDb, SimEventRow,
  SimRunRow, SpigotRow, WateringLogRow, YardRow, ZoneRow,
};
use chrono::{NaiveDate, NaiveDateTime};

fn dt(s: &str) -> NaiveDateTime {
  NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
}

async fn fresh_db() -> SimDb {
  let db = SimDb::connect(":memory:")
    .await
    .expect("open in-memory database");
  db.migrate().await.expect("apply migrations");
  db
}

fn property_row() -> PropertyRow {
  PropertyRow {
    id: "example-property".into(),
    name: "Example Property".into(),
    climate_zone: "portland-or".into(),
    lot_area_sq_ft: 6000.0,
  }
}

#[tokio::test]
async fn migrate_is_idempotent() {
  let db = fresh_db().await;
  // Running migrate a second time should be a no-op and not error.
  db.migrate().await.expect("re-run migrate");
}

#[tokio::test]
async fn property_inserts_and_reads_back() {
  let db = fresh_db().await;
  let p = property_row();
  db.insert_property(&p).await.expect("insert");
  let back = db
    .fetch_property(&p.id)
    .await
    .expect("fetch")
    .expect("present");
  assert_eq!(back, p);
  let missing = db.fetch_property("nope").await.expect("fetch");
  assert!(missing.is_none());
}

#[tokio::test]
async fn full_graph_round_trips() {
  let db = fresh_db().await;
  let p = property_row();
  db.insert_property(&p).await.expect("property");

  db.insert_yard(&YardRow {
    id: "yard-a".into(),
    property_id: p.id.clone(),
    name: "Yard A".into(),
    area_sq_ft: 2500.0,
  })
  .await
  .expect("yard");

  db.insert_spigot(&SpigotRow {
    id: "spigot-a".into(),
    property_id: p.id.clone(),
    mains_pressure_psi: 60.0,
    notes: Some("behind the garage".into()),
  })
  .await
  .expect("spigot");

  db.insert_manifold(&ManifoldRow {
    id: "manifold-a".into(),
    property_id: p.id.clone(),
    model_id: "generic-3zone-manifold".into(),
    spigot_id: "spigot-a".into(),
    zone_capacity: 3,
  })
  .await
  .expect("manifold");

  let zone_row = ZoneRow {
    id: "zone-a-veggies".into(),
    property_id: p.id.clone(),
    yard_id: "yard-a".into(),
    manifold_id: "manifold-a".into(),
    plant_kind: "veggie-bed".into(),
    emitter_spec_id: "inline-drip-12in".into(),
    soil_type_id: "silty-clay-loam".into(),
    area_sq_ft: 50.0,
    notes: None,
  };
  db.insert_zone(&zone_row).await.expect("zone");

  db.insert_plant(&PlantRow {
    id: "tomato-row-1".into(),
    property_id: p.id.clone(),
    zone_id: zone_row.id.clone(),
    species_id: "tomato-sungold".into(),
    planted_on: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
    water_need_ml_per_day: 1500.0,
    notes: Some("staked".into()),
  })
  .await
  .expect("plant");

  db.insert_controller_instance(&ControllerInstanceRow {
    id: "controller-a".into(),
    property_id: p.id.clone(),
    model_id: "example-24v-controller".into(),
    zone_assignments_json: "[\"zone-a-veggies\"]".into(),
    notes: None,
  })
  .await
  .expect("controller");

  db.insert_sensor_instance(&SensorInstanceRow {
    id: "sensor-a".into(),
    property_id: p.id.clone(),
    model_id: "example-soil-sensor".into(),
    zone_id: zone_row.id.clone(),
    notes: None,
  })
  .await
  .expect("sensor");

  let reading_id = db
    .insert_sensor_reading(&SensorReadingRow {
      id: 0,
      property_id: p.id.clone(),
      zone_id: zone_row.id.clone(),
      reading_kind: "soil-vwc".into(),
      value: 0.31,
      taken_at: dt("2026-07-01 06:00:00"),
    })
    .await
    .expect("reading");
  assert!(reading_id > 0, "sensor_reading id should be auto-assigned");

  let watering_id = db
    .insert_watering_log(&WateringLogRow {
      id: 0,
      property_id: p.id.clone(),
      zone_id: zone_row.id.clone(),
      started_at: dt("2026-07-01 06:00:00"),
      ended_at: Some(dt("2026-07-01 06:15:00")),
      duration_seconds: 900,
    })
    .await
    .expect("watering");
  assert!(watering_id > 0);

  db.insert_schedule_item(&ScheduleItemRow {
    id: 0,
    property_id: p.id.clone(),
    zone_id: zone_row.id.clone(),
    start_time_minutes_of_day: 6 * 60,
    duration_minutes: 15,
    day_mask: 127,
  })
  .await
  .expect("schedule");

  let sim_run_id = db
    .insert_sim_run(&SimRunRow {
      id: 0,
      property_id: p.id.clone(),
      scenario_name: "july-heatwave".into(),
      seed: 42,
      started_at: dt("2026-07-01 00:00:00"),
      completed_at: Some(dt("2026-07-08 00:00:00")),
      final_state_json: Some("{}".into()),
    })
    .await
    .expect("sim_run");

  db.insert_sim_event(&SimEventRow {
    id: 0,
    sim_run_id,
    instant_minutes: 60,
    event_kind: "valve-open".into(),
    payload_json: "{\"zone_id\":\"zone-a-veggies\",\"duration_minutes\":15}"
      .into(),
  })
  .await
  .expect("sim_event");

  db.insert_property_design(&PropertyDesignRow {
    id: 0,
    property_id: Some(p.id.clone()),
    requirements_json: "{}".into(),
    plan_json: "{}".into(),
    created_at: dt("2026-04-14 21:30:00"),
  })
  .await
  .expect("design");

  // Read-back: the zones_for_property helper proves the schema's
  // property_id columns line up, not just the types.
  let zones = db
    .zones_for_property(&p.id)
    .await
    .expect("zones_for_property");
  assert_eq!(zones.len(), 1);
  assert_eq!(zones[0], zone_row);
}

#[tokio::test]
async fn check_constraint_rejects_negative_area() {
  let db = fresh_db().await;
  let p = property_row();
  db.insert_property(&p).await.expect("property");
  let bad_yard = YardRow {
    id: "bad".into(),
    property_id: p.id.clone(),
    name: "Bad".into(),
    area_sq_ft: -1.0,
  };
  let err = db.insert_yard(&bad_yard).await.unwrap_err();
  assert!(
    format!("{err}").contains("insert_yard"),
    "error message must name the operation, got: {err}"
  );
}

#[tokio::test]
async fn foreign_key_rejects_orphan_zone() {
  let db = fresh_db().await;
  let p = property_row();
  db.insert_property(&p).await.expect("property");
  // No yard / manifold / spigot yet; inserting a zone violates the
  // foreign-key constraint on yard_id and manifold_id.
  let err = db
    .insert_zone(&ZoneRow {
      id: "orphan".into(),
      property_id: p.id.clone(),
      yard_id: "ghost".into(),
      manifold_id: "ghost".into(),
      plant_kind: "veggie-bed".into(),
      emitter_spec_id: "inline-drip-12in".into(),
      soil_type_id: "silty-clay-loam".into(),
      area_sq_ft: 10.0,
      notes: None,
    })
    .await
    .unwrap_err();
  assert!(format!("{err}").contains("insert_zone"));
}
