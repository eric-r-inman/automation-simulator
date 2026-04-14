//! Semantic error types for domain-model validation.
//!
//! Each error names the *process* that failed (`PropertyValidation`,
//! `ZoneValidation`), not just the mechanical fault.  The variant
//! names carry the information a reviewer needs to know what input
//! produced the failure and where to look: which property, which
//! zone, which missing or out-of-range field.
//!
//! These types intentionally do not wrap `std::io::Error` or
//! `toml::de::Error` — those belong to the loader (Phase 5), which
//! converts them into its own semantic errors before handing them
//! to the caller.  The domain-model layer sees already-deserialized
//! `*Raw` candidates and decides whether they are valid.

use thiserror::Error;

use super::id::{
  PlantId, PropertyId, SensorInstanceId, SpigotId, WeatherStationInstanceId,
  YardId, ZoneId,
};

// ── Property ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PropertyValidationError {
  #[error("property name must not be blank")]
  BlankPropertyName,

  #[error(
    "lot area must be positive, got {lot_area_sq_ft} sq ft for property {property}"
  )]
  NonPositiveLotArea {
    property: PropertyId,
    lot_area_sq_ft: f64,
  },

  #[error("property {0} must have at least one yard")]
  NoYards(PropertyId),

  #[error("property {0} must have at least one spigot")]
  NoSpigots(PropertyId),

  #[error("duplicate yard id {0} inside the same property")]
  DuplicateYardId(YardId),

  #[error("duplicate spigot id {0} inside the same property")]
  DuplicateSpigotId(SpigotId),

  #[error("yard {yard} area must be positive, got {area_sq_ft} sq ft")]
  NonPositiveYardArea { yard: YardId, area_sq_ft: f64 },

  #[error("yard name must not be blank in yard {0}")]
  BlankYardName(YardId),

  #[error("spigot {spigot} mains pressure must be positive, got {psi} psi")]
  NonPositiveMainsPressure { spigot: SpigotId, psi: f64 },

  #[error(
    "spigot {spigot} mains pressure {psi} psi is outside the \
     plausible residential range [20, 120] psi"
  )]
  ImplausibleMainsPressure { spigot: SpigotId, psi: f64 },
}

// ── Zone / manifold ──────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ZoneValidationError {
  #[error("duplicate zone id {0}")]
  DuplicateZoneId(ZoneId),

  #[error("zone {zone} references unknown yard {yard}")]
  ZoneYardNotFound { zone: ZoneId, yard: YardId },

  #[error("zone {zone} references unknown spigot {spigot}")]
  ZoneSpigotNotFound { zone: ZoneId, spigot: SpigotId },

  #[error("zone {zone} area must be positive, got {area_sq_ft} sq ft")]
  NonPositiveZoneArea { zone: ZoneId, area_sq_ft: f64 },

  #[error("duplicate manifold id inside the same property")]
  DuplicateManifoldId,

  #[error("manifold capacity must be positive, got {capacity} zones")]
  NonPositiveManifoldCapacity { capacity: i64 },

  #[error(
    "manifold has {assigned} zones assigned but its capacity is \
     only {capacity}"
  )]
  ManifoldOverCapacity { assigned: usize, capacity: i64 },

  #[error("manifold references unknown spigot {spigot}")]
  ManifoldSpigotNotFound { spigot: SpigotId },
}

// ── Plant ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PlantValidationError {
  #[error("duplicate plant id {0}")]
  DuplicatePlantId(PlantId),

  #[error("plant {plant} references unknown zone {zone}")]
  PlantZoneNotFound { plant: PlantId, zone: ZoneId },

  #[error(
    "plant {plant} water need must be positive, got \
     {water_need_ml_per_day} mL/day"
  )]
  NonPositiveWaterNeed {
    plant: PlantId,
    water_need_ml_per_day: f64,
  },

  #[error(
    "plant {plant} water need {water_need_ml_per_day} mL/day is \
     above the plausible residential upper bound of 50 000 mL/day"
  )]
  ImplausibleWaterNeed {
    plant: PlantId,
    water_need_ml_per_day: f64,
  },
}

// ── Hardware instances ───────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum HardwareValidationError {
  #[error("duplicate controller instance id")]
  DuplicateControllerId,

  #[error(
    "controller assigns unknown zone {0} to one of its output \
     channels"
  )]
  ControllerAssignsUnknownZone(ZoneId),

  #[error("controller assigns the same zone to multiple output channels: {0}")]
  ControllerDoubleAssignedZone(ZoneId),

  #[error("duplicate sensor instance id {0}")]
  DuplicateSensorId(SensorInstanceId),

  #[error("sensor {sensor} references unknown zone {zone}")]
  SensorZoneNotFound {
    sensor: SensorInstanceId,
    zone: ZoneId,
  },

  #[error("duplicate weather station instance id {0}")]
  DuplicateWeatherStationId(WeatherStationInstanceId),
}

// ── Scenario ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ScenarioValidationError {
  #[error("scenario name must not be blank")]
  BlankScenarioName,

  #[error(
    "scenario duration must be positive, got {duration_minutes} minutes"
  )]
  NonPositiveDuration { duration_minutes: i64 },

  #[error(
    "scenario weather override at offset {offset_minutes} exceeds \
     scenario duration {duration_minutes} minutes"
  )]
  WeatherOverrideBeyondDuration {
    offset_minutes: i64,
    duration_minutes: i64,
  },

  #[error(
    "scenario manual intervention at offset {offset_minutes} \
     exceeds scenario duration {duration_minutes} minutes"
  )]
  InterventionBeyondDuration {
    offset_minutes: i64,
    duration_minutes: i64,
  },

  #[error("scenario manual intervention references unknown zone {0}")]
  InterventionUnknownZone(ZoneId),
}
