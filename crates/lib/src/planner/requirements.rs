//! Property requirements: the user's description of what they want.
//!
//! The planner takes a `PropertyRequirements`, looks at the
//! catalog, and emits ranked `PropertyPlan` candidates.  Keep this
//! type tight: it's the user-facing contract, easy to author by
//! hand or generate from the v0.3 designer UI.

use serde::{Deserialize, Serialize};

use crate::sim::zone::PlantKind;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyRequirements {
  /// Stable id for the resulting property (the planner copies this
  /// straight onto the candidate `Property`).
  pub property_id: String,
  pub property_name: String,
  pub climate_zone: String,
  pub yards: Vec<YardRequirement>,
  /// Soft cost cap.  Plans over budget are still returned but
  /// pushed down the ranking; `None` means no preference.
  #[serde(default)]
  pub budget_usd: Option<f64>,
  #[serde(default)]
  pub prefer_smart_controller: bool,
  #[serde(default)]
  pub require_pressure_compensating: bool,
  /// Soil type id (must exist in the catalog).  Defaults to
  /// silty-clay-loam, the same default the example fixture uses.
  #[serde(default = "default_soil_type")]
  pub soil_type_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YardRequirement {
  pub id: String,
  pub name: String,
  pub area_sq_ft: f64,
  pub mains_pressure_psi: f64,
  pub zones: Vec<ZoneRequirement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneRequirement {
  /// Used to build the final zone id as
  /// `{yard.id}-{name_suffix}`.  Keep short and slug-friendly.
  pub name_suffix: String,
  pub plant_kind: PlantKind,
  pub area_sq_ft: f64,
}

fn default_soil_type() -> String {
  "silty-clay-loam".to_string()
}

impl PropertyRequirements {
  pub fn total_zone_count(&self) -> usize {
    self.yards.iter().map(|y| y.zones.len()).sum()
  }
}
