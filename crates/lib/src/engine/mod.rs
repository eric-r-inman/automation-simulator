//! Deterministic simulation engine.
//!
//! The engine consumes the domain types from [`crate::sim`] plus the
//! datasheet constants from [`crate::catalog`] and advances a world
//! forward in discrete one-minute sub-steps.  Every source of
//! non-determinism passes through an explicit seeded RNG so two runs
//! with the same inputs produce byte-identical state and sample
//! histories — that is the contract the snapshot tests rely on.
//!
//! Module layout mirrors the phase-3 plan: one file per concern.
//!
//! - [`clock`] provides the simulated time abstraction.
//! - [`weather`] turns a climatology lookup and a seeded RNG into a
//!   per-instant weather sample, with optional scripted overrides.
//! - [`soil`] carries the per-zone soil-moisture ODE that turns
//!   irrigation inflow, evapotranspiration, and drainage into a VWC
//!   update each sub-step.
//! - [`flow`] computes the per-zone irrigation inflow while a valve
//!   is open, drawing on the catalog's emitter specs.
//! - [`world`] is the top-level [`SimWorld`] that owns the clock,
//!   weather model, and per-zone state, and exposes
//!   [`SimWorld::advance`].

pub mod clock;
pub mod flow;
pub mod soil;
pub mod weather;
pub mod world;

pub use clock::{SimClock, SimDuration, SimInstant};
pub use flow::{derive_emitter_count, zone_inflow_mm_per_hour};
pub use soil::{soil_step, SoilParams, SoilState, SoilUpdate};
pub use weather::{Climatology, MonthlyNormal, WeatherModel, WeatherSample};
pub use world::{SensorSample, SimWorld, SimWorldError, ValveState};
