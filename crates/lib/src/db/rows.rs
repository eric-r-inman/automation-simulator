//! Row types that mirror the SQLite schema in `migrations/`.
//!
//! These are deliberately separate from the domain types in
//! [`crate::sim`].  A domain `Property` is a validated, shape-safe
//! Rust value; a [`PropertyRow`] is whatever the database gave us.
//! Mapping the two is a one-liner per direction once the schema is
//! stable; keeping them distinct means a schema migration can add a
//! column without forcing every domain-model consumer to change.

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ── Property + geometry ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct PropertyRow {
  pub id: String,
  pub name: String,
  pub climate_zone: String,
  pub lot_area_sq_ft: f64,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct YardRow {
  pub id: String,
  pub property_id: String,
  pub name: String,
  pub area_sq_ft: f64,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct SpigotRow {
  pub id: String,
  pub property_id: String,
  pub mains_pressure_psi: f64,
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct ManifoldRow {
  pub id: String,
  pub property_id: String,
  pub model_id: String,
  pub spigot_id: String,
  pub zone_capacity: i64,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct ZoneRow {
  pub id: String,
  pub property_id: String,
  pub yard_id: String,
  pub manifold_id: String,
  pub plant_kind: String,
  pub emitter_spec_id: String,
  pub soil_type_id: String,
  pub area_sq_ft: f64,
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct PlantRow {
  pub id: String,
  pub property_id: String,
  pub zone_id: String,
  pub species_id: String,
  pub planted_on: NaiveDate,
  pub water_need_ml_per_day: f64,
  pub notes: Option<String>,
}

// ── Hardware instances ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct ControllerInstanceRow {
  pub id: String,
  pub property_id: String,
  pub model_id: String,
  /// Raw JSON-encoded array of zone ids.  Use
  /// [`ControllerInstanceRow::decode_zone_assignments`] to get a
  /// typed `Vec<String>` back.
  pub zone_assignments_json: String,
  pub notes: Option<String>,
}

impl ControllerInstanceRow {
  pub fn decode_zone_assignments(
    &self,
  ) -> Result<Vec<String>, serde_json::Error> {
    serde_json::from_str(&self.zone_assignments_json)
  }
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct SensorInstanceRow {
  pub id: String,
  pub property_id: String,
  pub model_id: String,
  pub zone_id: String,
  pub notes: Option<String>,
}

// ── History + schedule ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct SensorReadingRow {
  pub id: i64,
  pub property_id: String,
  pub zone_id: String,
  pub reading_kind: String,
  pub value: f64,
  pub taken_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct WateringLogRow {
  pub id: i64,
  pub property_id: String,
  pub zone_id: String,
  pub started_at: NaiveDateTime,
  pub ended_at: Option<NaiveDateTime>,
  pub duration_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct ScheduleItemRow {
  pub id: i64,
  pub property_id: String,
  pub zone_id: String,
  pub start_time_minutes_of_day: i64,
  pub duration_minutes: i64,
  /// Seven-bit weekday mask, Monday = 1 through Sunday = 64.  0 is
  /// "paused", 127 is "every day".
  pub day_mask: i64,
}

// ── Simulation runs + events + designs ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct SimRunRow {
  pub id: i64,
  pub property_id: String,
  pub scenario_name: String,
  pub seed: i64,
  pub started_at: NaiveDateTime,
  pub completed_at: Option<NaiveDateTime>,
  pub final_state_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct SimEventRow {
  pub id: i64,
  pub sim_run_id: i64,
  pub instant_minutes: i64,
  pub event_kind: String,
  pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize)]
pub struct PropertyDesignRow {
  pub id: i64,
  pub property_id: Option<String>,
  pub requirements_json: String,
  pub plan_json: String,
  pub created_at: NaiveDateTime,
}
