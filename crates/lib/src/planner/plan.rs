//! Candidate plan output.  Each `PropertyPlan` is a property
//! definition the user can simulate against, paired with a bill
//! of materials and a short rationale.

use serde::{Deserialize, Serialize};

use crate::seed::PropertyBundle;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomLine {
  pub category: String,
  pub catalog_id: String,
  pub display_name: String,
  pub manufacturer: String,
  pub quantity: i64,
  pub unit_price_usd: f64,
  pub line_total_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bom {
  pub lines: Vec<BomLine>,
  pub total_usd: f64,
}

impl Bom {
  pub fn from_lines(lines: Vec<BomLine>) -> Self {
    let total = lines.iter().map(|l| l.line_total_usd).sum();
    Self {
      lines,
      total_usd: total,
    }
  }
}

/// One candidate plan ranked by [`crate::planner::recommend`].
/// `bundle` is a fully validated `PropertyBundle` that the
/// existing seed loader / SimWorld can consume directly.
/// `bom` itemises the cost; `score` is the (higher = better)
/// ranking number; `rationale` is a small list of strings the UI
/// can display as bullet points.
#[derive(Debug, Clone, Serialize)]
pub struct PropertyPlan {
  pub plan_id: String,
  #[serde(skip)]
  pub bundle: PropertyBundle,
  pub bom: Bom,
  pub score: f64,
  pub rationale: Vec<String>,
  pub controller_model_id: String,
  pub controller_max_zones: i64,
}
