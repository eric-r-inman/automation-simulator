-- Initial schema.
--
-- Multi-property from day one: every row below carries property_id
-- so v0.3 can store more than one property per database without a
-- second migration.  v0.1 keeps only one row in the property table.
--
-- All tables are STRICT so SQLite enforces column types.  Timestamps
-- are stored as ISO-8601 strings (chrono's default), dates as
-- YYYY-MM-DD strings, and enums as kebab-case text (matching serde's
-- rename_all configuration on the Rust side).

PRAGMA foreign_keys = ON;

CREATE TABLE property (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  climate_zone TEXT NOT NULL,
  lot_area_sq_ft REAL NOT NULL CHECK (lot_area_sq_ft > 0)
) STRICT;

CREATE TABLE yard (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  area_sq_ft REAL NOT NULL CHECK (area_sq_ft > 0)
) STRICT;

CREATE TABLE spigot (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  mains_pressure_psi REAL NOT NULL CHECK (mains_pressure_psi > 0),
  notes TEXT
) STRICT;

CREATE TABLE manifold (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  spigot_id TEXT NOT NULL REFERENCES spigot(id) ON DELETE RESTRICT,
  zone_capacity INTEGER NOT NULL CHECK (zone_capacity > 0)
) STRICT;

CREATE TABLE zone (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  yard_id TEXT NOT NULL REFERENCES yard(id) ON DELETE RESTRICT,
  manifold_id TEXT NOT NULL REFERENCES manifold(id) ON DELETE RESTRICT,
  plant_kind TEXT NOT NULL,
  emitter_spec_id TEXT NOT NULL,
  soil_type_id TEXT NOT NULL,
  area_sq_ft REAL NOT NULL CHECK (area_sq_ft > 0),
  notes TEXT
) STRICT;

CREATE TABLE plant (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  zone_id TEXT NOT NULL REFERENCES zone(id) ON DELETE CASCADE,
  species_id TEXT NOT NULL,
  planted_on TEXT NOT NULL,
  water_need_ml_per_day REAL NOT NULL CHECK (water_need_ml_per_day > 0),
  notes TEXT
) STRICT;

CREATE TABLE controller_instance (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  -- JSON array of zone ids in channel order; the server deserializes
  -- this on read rather than joining through a second table.
  zone_assignments_json TEXT NOT NULL,
  notes TEXT
) STRICT;

CREATE TABLE sensor_instance (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  zone_id TEXT NOT NULL REFERENCES zone(id) ON DELETE CASCADE,
  notes TEXT
) STRICT;

CREATE TABLE weather_station_instance (
  id TEXT PRIMARY KEY NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  -- NULL means the station is property-level, not tied to a yard.
  yard_id TEXT REFERENCES yard(id) ON DELETE RESTRICT,
  notes TEXT
) STRICT;

CREATE TABLE sensor_reading (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  zone_id TEXT NOT NULL,
  reading_kind TEXT NOT NULL,
  value REAL NOT NULL,
  taken_at TEXT NOT NULL
) STRICT;

CREATE INDEX ix_sensor_reading_by_zone_time
  ON sensor_reading(property_id, zone_id, taken_at);

CREATE TABLE watering_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  zone_id TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  duration_seconds INTEGER NOT NULL CHECK (duration_seconds >= 0)
) STRICT;

CREATE INDEX ix_watering_log_by_zone_time
  ON watering_log(property_id, zone_id, started_at);

CREATE TABLE schedule_item (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  zone_id TEXT NOT NULL,
  start_time_minutes_of_day INTEGER NOT NULL
    CHECK (start_time_minutes_of_day BETWEEN 0 AND 1439),
  duration_minutes INTEGER NOT NULL CHECK (duration_minutes > 0),
  -- Seven-bit weekday mask; Monday = 1, Sunday = 64.  A value of
  -- 127 runs every day.  0 is a reserved "never" used by the UI to
  -- pause a schedule without deleting it.
  day_mask INTEGER NOT NULL CHECK (day_mask BETWEEN 0 AND 127)
) STRICT;

CREATE TABLE sim_run (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  property_id TEXT NOT NULL REFERENCES property(id) ON DELETE CASCADE,
  scenario_name TEXT NOT NULL,
  seed INTEGER NOT NULL,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  final_state_json TEXT
) STRICT;

CREATE TABLE sim_event (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  sim_run_id INTEGER NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  instant_minutes INTEGER NOT NULL,
  event_kind TEXT NOT NULL,
  payload_json TEXT NOT NULL
) STRICT;

CREATE INDEX ix_sim_event_by_run ON sim_event(sim_run_id, instant_minutes);

CREATE TABLE property_design (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  -- NULL during drafting: designs are authored before a property
  -- exists.  v0.3 commits a design into a property row and back-
  -- fills this column.
  property_id TEXT REFERENCES property(id) ON DELETE CASCADE,
  requirements_json TEXT NOT NULL,
  plan_json TEXT NOT NULL,
  created_at TEXT NOT NULL
) STRICT;
