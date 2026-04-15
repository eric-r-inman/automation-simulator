//! The catalog's model types.
//!
//! One struct per category of hardware or biology.  Each model is a
//! flat POD of datasheet values the simulator actually needs.  The
//! compliance test in `crates/lib/tests/no_hardcoded_property_names.rs`
//! keeps brand names out of the source tree; seed data lives in
//! `data/catalog/*.toml` and the loader resolves ids at runtime.
//!
//! These types are additive-only.  Growing the catalog to support a
//! new controller, sensor, or species means adding a row to the
//! relevant TOML — never touching these structs.

use serde::{Deserialize, Serialize};

use crate::sim::id::{
  BackflowPreventerModelId, ComputeHostModelId, ControllerModelId,
  DripLineModelId, EmitterSpecId, ManifoldModelId, PressureRegulatorModelId,
  SensorModelId, SoilTypeId, SpeciesId, ValveModelId, WeatherStationModelId,
};
use crate::sim::zone::PlantKind;

// ── Controller ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControllerModel {
  pub id: ControllerModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  pub max_zones: i64,
  /// Nominal coil drive voltage for the solenoid valves this
  /// controller switches.  Typical residential values are 24 V AC.
  pub valve_voltage_ac: f64,
  /// True for Wi-Fi + weather-aware "smart" controllers — used by
  /// the planner to satisfy `prefer_smart_controller` requests.
  #[serde(default)]
  pub is_smart: bool,
  /// True for controllers that do not ship with their own compute
  /// and instead mount on a host SBC (e.g. a Raspberry Pi HAT).
  /// When set, the planner adds a compute-host line to the BOM so
  /// the BOM is actually buildable.
  #[serde(default)]
  pub requires_compute_host: bool,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Compute host ─────────────────────────────────────────────────────────────

/// A small single-board-computer (SBC) that hosts a controller
/// HAT or runs the simulator itself.  Rows live in
/// `data/catalog/compute-hosts.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputeHostModel {
  pub id: ComputeHostModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  /// Memory the SBC ships with, in megabytes.  The planner does
  /// not currently filter on this — it exists so the BOM line can
  /// show something meaningful about which variant was picked.
  pub memory_mb: i64,
  /// True when this host carries a 40-pin header compatible with
  /// the Raspberry Pi GPIO pinout, i.e. it can accept a Pi HAT.
  #[serde(default)]
  pub supports_raspberry_pi_hat: bool,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Sensor ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SensorKind {
  SoilMoisture,
  Flow,
  Pressure,
  Temperature,
  Gateway,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SensorModel {
  pub id: SensorModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  pub kind: SensorKind,
  /// Optional reference to a gateway sensor-model this sensor relays
  /// through.  Loader cross-validates that the id exists.
  #[serde(default)]
  pub gateway_model_id: Option<SensorModelId>,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Weather station ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeatherStationModel {
  pub id: WeatherStationModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  pub measures_temperature: bool,
  pub measures_humidity: bool,
  pub measures_wind: bool,
  pub measures_rain: bool,
  pub measures_solar: bool,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Manifold and valves ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifoldModel {
  pub id: ManifoldModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  /// Maximum number of zones this manifold can serve.
  pub zone_capacity: i64,
  /// Coil drive voltage of the bundled solenoid valves.
  pub valve_voltage_ac: f64,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValveModel {
  pub id: ValveModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  pub voltage_ac: f64,
  pub coil_current_a: f64,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Emitter ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmitterShape {
  /// Point emitter mounted at a stake next to a plant.
  PointEmitter,
  /// Tubing with emitters pre-installed at fixed spacing.
  InlineDrip,
  /// Adjustable-arc micro-spray head on a stake.
  MicroSpray,
  /// Bubbler for tree basins.
  Bubbler,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmitterSpec {
  pub id: EmitterSpecId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd_per_100: f64,
  pub shape: EmitterShape,
  /// Nominal flow at the emitter's rated pressure, gallons per hour.
  pub flow_gph: f64,
  /// Minimum inlet pressure for reliable operation, psi.
  pub min_inlet_psi: f64,
  pub pressure_compensating: bool,
  /// For inline drip: spacing between built-in emitters, inches.
  /// `None` for point emitters and sprays.
  #[serde(default)]
  pub inline_spacing_inches: Option<f64>,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Pressure regulator / backflow preventer / drip line ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PressureRegulatorModel {
  pub id: PressureRegulatorModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  pub input_psi_min: f64,
  pub input_psi_max: f64,
  pub output_psi: f64,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackflowKind {
  /// Basic atmospheric vacuum breaker (AVB).
  AtmosphericVacuumBreaker,
  /// Pressure vacuum breaker (PVB).
  PressureVacuumBreaker,
  /// Double check valve assembly (DCVA).
  DoubleCheckValve,
  /// Reduced-pressure zone assembly (RPZ) — highest protection.
  ReducedPressureZone,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackflowPreventerModel {
  pub id: BackflowPreventerModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd: f64,
  pub kind: BackflowKind,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DripLineModel {
  pub id: DripLineModelId,
  pub name: String,
  pub manufacturer: String,
  pub price_usd_per_foot: f64,
  pub outer_diameter_inches: f64,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Species ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Species {
  pub id: SpeciesId,
  pub common_name: String,
  pub scientific_name: String,
  pub kind: PlantKind,
  /// Baseline daily water need for a mature plant under typical
  /// summer conditions, millilitres per day.  Zone and scenario
  /// modifiers apply at simulation time.
  pub water_need_base_ml_per_day: f64,
  pub root_depth_inches: f64,
  /// Approximate mature canopy area, square feet — used by the
  /// catalog-driven recommender in v0.3.
  pub mature_size_sq_ft: f64,
  /// Inclusive USDA hardiness range this species tolerates.
  pub hardiness_zone_min: i64,
  pub hardiness_zone_max: i64,
  #[serde(default)]
  pub notes: Option<String>,
}

// ── Soil type ────────────────────────────────────────────────────────────────

/// The physics constants the soil-moisture ODE will consume in
/// Phase 3.  Volumetric water content (VWC) values are dimensionless
/// ratios in [0, 1].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoilType {
  pub id: SoilTypeId,
  pub name: String,
  /// VWC at which the soil is fully saturated and any additional
  /// water drains beyond the root zone.
  pub saturation_vwc: f64,
  /// VWC the soil settles at under gravity alone once excess water
  /// has drained — the upper bound of plant-available water.
  pub field_capacity_vwc: f64,
  /// VWC below which plants can no longer extract water.
  pub wilting_point_vwc: f64,
  /// Saturated hydraulic conductivity, mm/hr — used to size the
  /// drainage term in the moisture ODE.
  pub saturated_hydraulic_conductivity_mm_per_hr: f64,
  #[serde(default)]
  pub notes: Option<String>,
}
