//! SQLite persistence layer.
//!
//! [`SimDb`] is the single handle the rest of the crate holds onto.
//! It owns a sqlx connection pool, exposes migration, and offers a
//! small set of insert/select helpers for each row type.  Queries
//! here use the runtime `sqlx::query` / `sqlx::query_as` builders
//! (not the compile-time `query!` macros) so the build does not
//! need a live database.  We can adopt the macros incrementally
//! once the schema settles and Phase 7 wants the compile-time
//! checks.
//!
//! Paths accepted by [`SimDb::connect`]:
//!
//! - An ordinary filesystem path (absolute or relative).  The parent
//!   directory is created if it does not exist; the SQLite file is
//!   created on first connect.
//! - The special literal `":memory:"` for ephemeral in-process
//!   databases — handy for round-trip tests.

pub mod errors;
pub mod rows;

pub use errors::{DbOpenError, MigrationError, QueryError};
pub use rows::{
  ControllerInstanceRow, ManifoldRow, PlantRow, PropertyDesignRow, PropertyRow,
  ScheduleItemRow, SensorInstanceRow, SensorReadingRow, SimEventRow, SimRunRow,
  SpigotRow, WateringLogRow, WeatherStationInstanceRow, YardRow, ZoneRow,
};

use sqlx::sqlite::{
  SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::str::FromStr;

static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Handle to an open SQLite database.  Cheap to clone (the inner
/// `SqlitePool` is `Arc`-backed).
#[derive(Debug, Clone)]
pub struct SimDb {
  pool: SqlitePool,
}

impl SimDb {
  /// Open (or create) a SQLite database at the given path.  Accepts
  /// a filesystem path or the literal `":memory:"`.  Foreign keys
  /// are enabled on every connection in the pool; journal mode is
  /// set to WAL for the non-memory case.
  pub async fn connect(path: impl AsRef<Path>) -> Result<Self, DbOpenError> {
    let path = path.as_ref();
    let is_memory = path.as_os_str() == ":memory:";

    // Create the parent directory for on-disk paths so a first-time
    // connect does not fail when the user has not pre-created the
    // directory structure.
    if !is_memory {
      if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
          std::fs::create_dir_all(parent).map_err(|source| {
            DbOpenError::CreateParentDir {
              path: parent.to_path_buf(),
              source,
            }
          })?;
        }
      }
    }

    let mut options = if is_memory {
      SqliteConnectOptions::from_str("sqlite::memory:").map_err(|e| {
        DbOpenError::InvalidUrl {
          url: ":memory:".into(),
          reason: e.to_string(),
        }
      })?
    } else {
      SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
    };
    options = options.foreign_keys(true);

    // For in-memory the pool must be size-one so every query hits
    // the same private database; otherwise the pool hands out new
    // empty in-memory DBs round-robin and migrations "disappear".
    let max_connections = if is_memory { 1 } else { 5 };

    let pool = SqlitePoolOptions::new()
      .max_connections(max_connections)
      .connect_with(options)
      .await
      .map_err(|source| DbOpenError::Connect {
        path: path_for_error(path),
        source,
      })?;

    Ok(Self { pool })
  }

  /// Apply pending migrations embedded at compile time from
  /// `crates/lib/migrations/`.  Idempotent — running the migrator
  /// against an already-migrated database is a no-op.
  pub async fn migrate(&self) -> Result<(), MigrationError> {
    MIGRATIONS
      .run(&self.pool)
      .await
      .map_err(MigrationError::Apply)
  }

  pub fn pool(&self) -> &SqlitePool {
    &self.pool
  }
}

fn path_for_error(p: &Path) -> PathBuf {
  if p.as_os_str() == ":memory:" {
    PathBuf::from(":memory:")
  } else {
    p.to_path_buf()
  }
}

// ── Typed insert / select helpers ────────────────────────────────────────────
//
// Each helper names the *operation* in the QueryError it produces,
// not just the table.  Keeps diagnostics specific when a migration
// drift causes an INSERT to fail.

impl SimDb {
  pub async fn insert_property(
    &self,
    row: &PropertyRow,
  ) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO property (id, name, climate_zone, lot_area_sq_ft)
         VALUES (?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.name)
    .bind(&row.climate_zone)
    .bind(row.lot_area_sq_ft)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_property", e))?;
    Ok(())
  }

  pub async fn fetch_property(
    &self,
    id: &str,
  ) -> Result<Option<PropertyRow>, QueryError> {
    sqlx::query_as::<_, PropertyRow>(
      r#"SELECT id, name, climate_zone, lot_area_sq_ft
         FROM property WHERE id = ?"#,
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("fetch_property", e))
  }

  pub async fn insert_yard(&self, row: &YardRow) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO yard (id, property_id, name, area_sq_ft)
         VALUES (?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(&row.name)
    .bind(row.area_sq_ft)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_yard", e))?;
    Ok(())
  }

  pub async fn insert_spigot(&self, row: &SpigotRow) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO spigot (id, property_id, mains_pressure_psi, notes)
         VALUES (?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(row.mains_pressure_psi)
    .bind(&row.notes)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_spigot", e))?;
    Ok(())
  }

  pub async fn insert_manifold(
    &self,
    row: &ManifoldRow,
  ) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO manifold
           (id, property_id, model_id, spigot_id, zone_capacity)
         VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(&row.model_id)
    .bind(&row.spigot_id)
    .bind(row.zone_capacity)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_manifold", e))?;
    Ok(())
  }

  pub async fn insert_zone(&self, row: &ZoneRow) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO zone
           (id, property_id, yard_id, manifold_id, plant_kind,
            emitter_spec_id, soil_type_id, area_sq_ft, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(&row.yard_id)
    .bind(&row.manifold_id)
    .bind(&row.plant_kind)
    .bind(&row.emitter_spec_id)
    .bind(&row.soil_type_id)
    .bind(row.area_sq_ft)
    .bind(&row.notes)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_zone", e))?;
    Ok(())
  }

  pub async fn insert_plant(&self, row: &PlantRow) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO plant
           (id, property_id, zone_id, species_id, planted_on,
            water_need_ml_per_day, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(&row.zone_id)
    .bind(&row.species_id)
    .bind(row.planted_on)
    .bind(row.water_need_ml_per_day)
    .bind(&row.notes)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_plant", e))?;
    Ok(())
  }

  pub async fn insert_controller_instance(
    &self,
    row: &ControllerInstanceRow,
  ) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO controller_instance
           (id, property_id, model_id, zone_assignments_json, notes)
         VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(&row.model_id)
    .bind(&row.zone_assignments_json)
    .bind(&row.notes)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_controller_instance", e))?;
    Ok(())
  }

  pub async fn insert_sensor_instance(
    &self,
    row: &SensorInstanceRow,
  ) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO sensor_instance
           (id, property_id, model_id, zone_id, notes)
         VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(&row.model_id)
    .bind(&row.zone_id)
    .bind(&row.notes)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_sensor_instance", e))?;
    Ok(())
  }

  pub async fn insert_weather_station_instance(
    &self,
    row: &WeatherStationInstanceRow,
  ) -> Result<(), QueryError> {
    sqlx::query(
      r#"INSERT INTO weather_station_instance
           (id, property_id, model_id, yard_id, notes)
         VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&row.id)
    .bind(&row.property_id)
    .bind(&row.model_id)
    .bind(&row.yard_id)
    .bind(&row.notes)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_weather_station_instance", e))?;
    Ok(())
  }

  pub async fn insert_sensor_reading(
    &self,
    row: &SensorReadingRow,
  ) -> Result<i64, QueryError> {
    // Auto-increment ids are written by SQLite, so the caller's row
    // id is ignored and the new id is returned so the caller can
    // carry it onward.
    let res = sqlx::query(
      r#"INSERT INTO sensor_reading
           (property_id, zone_id, reading_kind, value, taken_at)
         VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&row.property_id)
    .bind(&row.zone_id)
    .bind(&row.reading_kind)
    .bind(row.value)
    .bind(row.taken_at)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_sensor_reading", e))?;
    Ok(res.last_insert_rowid())
  }

  pub async fn insert_watering_log(
    &self,
    row: &WateringLogRow,
  ) -> Result<i64, QueryError> {
    let res = sqlx::query(
      r#"INSERT INTO watering_log
           (property_id, zone_id, started_at, ended_at, duration_seconds)
         VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&row.property_id)
    .bind(&row.zone_id)
    .bind(row.started_at)
    .bind(row.ended_at)
    .bind(row.duration_seconds)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_watering_log", e))?;
    Ok(res.last_insert_rowid())
  }

  pub async fn insert_schedule_item(
    &self,
    row: &ScheduleItemRow,
  ) -> Result<i64, QueryError> {
    let res = sqlx::query(
      r#"INSERT INTO schedule_item
           (property_id, zone_id, start_time_minutes_of_day,
            duration_minutes, day_mask)
         VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&row.property_id)
    .bind(&row.zone_id)
    .bind(row.start_time_minutes_of_day)
    .bind(row.duration_minutes)
    .bind(row.day_mask)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_schedule_item", e))?;
    Ok(res.last_insert_rowid())
  }

  pub async fn insert_sim_run(
    &self,
    row: &SimRunRow,
  ) -> Result<i64, QueryError> {
    let res = sqlx::query(
      r#"INSERT INTO sim_run
           (property_id, scenario_name, seed, started_at, completed_at,
            final_state_json)
         VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&row.property_id)
    .bind(&row.scenario_name)
    .bind(row.seed)
    .bind(row.started_at)
    .bind(row.completed_at)
    .bind(&row.final_state_json)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_sim_run", e))?;
    Ok(res.last_insert_rowid())
  }

  pub async fn insert_sim_event(
    &self,
    row: &SimEventRow,
  ) -> Result<i64, QueryError> {
    let res = sqlx::query(
      r#"INSERT INTO sim_event
           (sim_run_id, instant_minutes, event_kind, payload_json)
         VALUES (?, ?, ?, ?)"#,
    )
    .bind(row.sim_run_id)
    .bind(row.instant_minutes)
    .bind(&row.event_kind)
    .bind(&row.payload_json)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_sim_event", e))?;
    Ok(res.last_insert_rowid())
  }

  pub async fn insert_property_design(
    &self,
    row: &PropertyDesignRow,
  ) -> Result<i64, QueryError> {
    let res = sqlx::query(
      r#"INSERT INTO property_design
           (property_id, requirements_json, plan_json, created_at)
         VALUES (?, ?, ?, ?)"#,
    )
    .bind(&row.property_id)
    .bind(&row.requirements_json)
    .bind(&row.plan_json)
    .bind(row.created_at)
    .execute(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("insert_property_design", e))?;
    Ok(res.last_insert_rowid())
  }

  pub async fn zones_for_property(
    &self,
    property_id: &str,
  ) -> Result<Vec<ZoneRow>, QueryError> {
    sqlx::query_as::<_, ZoneRow>(
      r#"SELECT id, property_id, yard_id, manifold_id, plant_kind,
                emitter_spec_id, soil_type_id, area_sq_ft, notes
         FROM zone WHERE property_id = ?"#,
    )
    .bind(property_id)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| QueryError::sqlx("zones_for_property", e))
  }
}
