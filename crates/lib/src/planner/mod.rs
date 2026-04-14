//! Catalog-driven planner: turn a `PropertyRequirements` into
//! ranked candidate `PropertyPlan`s.
//!
//! The planner is deterministic and catalog-driven — same inputs
//! and same catalog always produce the same output.  Brand names
//! never appear in planner source; smart-controller detection
//! reads the `is_smart` field on `ControllerModel` so the catalog
//! TOML remains the single source of truth for hardware facts.

pub mod errors;
pub mod plan;
pub mod recommend;
pub mod requirements;
pub mod scoring;

pub use errors::PlannerError;
pub use plan::{Bom, BomLine, PropertyPlan};
pub use recommend::recommend;
pub use requirements::{
  PropertyRequirements, YardRequirement, ZoneRequirement,
};
pub use scoring::score;
