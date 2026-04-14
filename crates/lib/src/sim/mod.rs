//! Simulation domain model.
//!
//! Everything in this module is pure: no I/O, no clock reads, no
//! randomness beyond an explicit seeded RNG.  That keeps the engine
//! deterministic (Phase 3 needs this for snapshot tests) and keeps
//! the types reusable from the CLI, the server, and future drivers
//! without pulling in async machinery.
//!
//! The domain is *catalog-driven*: nothing here knows the price of
//! a specific controller or the gallons-per-hour of a specific
//! emitter.  Hardware and plants carry string ids that reference
//! rows in a catalog loaded separately (Phase 2.5).  This way v0.3
//! grows the catalog without touching the domain types.
//!
//! Each public type has a sibling `*Raw` candidate type that mirrors
//! the TOML / JSON shape for untrusted input.  Validated types can
//! only be constructed via `TryFrom<*Raw>`, so any value of a public
//! type is already structurally valid.

pub mod errors;
pub mod hardware;
pub mod id;
pub mod plant;
pub mod property;
pub mod scenario;
pub mod zone;

pub use errors::{
  HardwareValidationError, PlantValidationError, PropertyValidationError,
  ScenarioValidationError, ZoneValidationError,
};
pub use hardware::{
  ControllerInstance, ControllerInstanceRaw, SensorInstance, SensorInstanceRaw,
  WeatherStationInstance, WeatherStationInstanceRaw,
};
pub use id::{
  BackflowPreventerModelId, ControllerInstanceId, ControllerModelId,
  DripLineModelId, EmitterSpecId, ManifoldInstanceId, ManifoldModelId, PlantId,
  PressureRegulatorModelId, PropertyId, SensorInstanceId, SensorModelId,
  SoilTypeId, SpeciesId, SpigotId, ValveModelId, WeatherStationInstanceId,
  WeatherStationModelId, YardId, ZoneId,
};
pub use plant::{Plant, PlantRaw};
pub use property::{Property, PropertyRaw, Spigot, SpigotRaw, Yard, YardRaw};
pub use scenario::{
  ManualIntervention, Scenario, ScenarioRaw, WeatherOverride,
};
pub use zone::{Manifold, ManifoldRaw, PlantKind, Zone, ZoneRaw};
