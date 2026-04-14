//! Semantic errors for property-fixture loading.
//!
//! Each variant names the property file, the problematic entity,
//! and — where applicable — the referenced id that could not be
//! resolved.  "Seed" here is the process of turning a property
//! TOML into validated domain types and inserting them into the
//! database; a failure can come from any of those steps.

use std::path::PathBuf;
use thiserror::Error;

use crate::db::{MigrationError, QueryError};
use crate::sim::errors::{
  HardwareValidationError, PlantValidationError, PropertyValidationError,
  ZoneValidationError,
};
use crate::sim::id::{
  ControllerInstanceId, ControllerModelId, EmitterSpecId, ManifoldInstanceId,
  PlantId, SensorInstanceId, SensorModelId, SoilTypeId, SpeciesId, SpigotId,
  WeatherStationInstanceId, WeatherStationModelId, YardId, ZoneId,
};

#[derive(Debug, Error)]
pub enum SeedError {
  #[error("failed to read property fixture at {path:?}: {source}")]
  FixtureFileRead {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },

  #[error("failed to parse property fixture at {path:?}: {source}")]
  FixtureFileParse {
    path: PathBuf,
    #[source]
    source: toml::de::Error,
  },

  #[error("property fixture validation failed: {0}")]
  PropertyInvalid(#[from] PropertyValidationError),

  #[error("property fixture zone/manifold validation failed: {0}")]
  ZoneInvalid(#[from] ZoneValidationError),

  #[error("property fixture plant validation failed: {0}")]
  PlantInvalid(#[from] PlantValidationError),

  #[error("property fixture hardware validation failed: {0}")]
  HardwareInvalid(#[from] HardwareValidationError),

  // ── Cross-references within the property file ─────────────────────────────
  #[error("manifold {manifold} references unknown spigot {spigot}")]
  ManifoldSpigotMissing {
    manifold: ManifoldInstanceId,
    spigot: SpigotId,
  },

  #[error("zone {zone} references unknown yard {yard}")]
  ZoneYardMissing { zone: ZoneId, yard: YardId },

  #[error("zone {zone} references unknown manifold {manifold}")]
  ZoneManifoldMissing {
    zone: ZoneId,
    manifold: ManifoldInstanceId,
  },

  #[error("plant {plant} references unknown zone {zone}")]
  PlantZoneMissing { plant: PlantId, zone: ZoneId },

  #[error("controller {controller} references unknown zone {zone}")]
  ControllerZoneMissing {
    controller: ControllerInstanceId,
    zone: ZoneId,
  },

  #[error("sensor {sensor} references unknown zone {zone}")]
  SensorZoneMissing {
    sensor: SensorInstanceId,
    zone: ZoneId,
  },

  #[error("weather station {station} references unknown yard {yard}")]
  WeatherStationYardMissing {
    station: WeatherStationInstanceId,
    yard: YardId,
  },

  #[error(
    "controller {controller} (model {model}) is wired to {assigned} zones but the model only supports {max_zones}"
  )]
  ControllerOverCapacity {
    controller: ControllerInstanceId,
    model: ControllerModelId,
    assigned: usize,
    max_zones: i64,
  },

  // ── Cross-references against the catalog ──────────────────────────────────
  #[error(
    "zone {zone} references soil type {soil_type} which is not in the catalog"
  )]
  UnknownSoilTypeRef { zone: ZoneId, soil_type: SoilTypeId },

  #[error(
    "zone {zone} references emitter spec {emitter} which is not in the catalog"
  )]
  UnknownEmitterRef {
    zone: ZoneId,
    emitter: EmitterSpecId,
  },

  #[error(
    "plant {plant} references species {species} which is not in the catalog"
  )]
  UnknownSpeciesRef { plant: PlantId, species: SpeciesId },

  #[error(
    "controller {controller} references model {model} which is not in the catalog"
  )]
  UnknownControllerModel {
    controller: ControllerInstanceId,
    model: ControllerModelId,
  },

  #[error(
    "sensor {sensor} references model {model} which is not in the catalog"
  )]
  UnknownSensorModel {
    sensor: SensorInstanceId,
    model: SensorModelId,
  },

  #[error(
    "weather station {station} references model {model} which is not in the catalog"
  )]
  UnknownWeatherStationModel {
    station: WeatherStationInstanceId,
    model: WeatherStationModelId,
  },

  // ── Persistence ───────────────────────────────────────────────────────────
  #[error("failed to apply database migrations before seeding: {0}")]
  PreMigrate(#[from] MigrationError),

  #[error("failed to write property row to database during seeding: {0}")]
  Insert(#[from] QueryError),
}
