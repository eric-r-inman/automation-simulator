//! Scoring rubric for candidate plans.
//!
//! Scores are in arbitrary "points"; higher is better.  The exact
//! numbers don't matter — what matters is that the same inputs
//! always produce the same relative ranking.  The score components
//! are independent so they can be tuned without touching the
//! recommender.

use crate::catalog::ControllerModel;

use super::plan::Bom;
use super::requirements::PropertyRequirements;

/// Baseline every plan gets; budget and smart-preference components
/// add or subtract from here.
pub const BASE_SCORE: f64 = 100.0;
/// Flat reward when the plan comes in at or under the stated
/// budget.  Absent budgets award nothing either way.
pub const BUDGET_WITHIN_REWARD: f64 = 25.0;
/// Penalty per dollar over budget.  Linear so "mildly over" still
/// ranks above "wildly over" without a special-case cliff.
pub const BUDGET_OVER_PENALTY_PER_USD: f64 = 0.05;
/// Reward when the user asked for a smart controller and got one.
pub const SMART_PREFERENCE_REWARD: f64 = 15.0;
/// Small reward that grows with headroom between the controller's
/// capacity and the requested zone count — rewards not maxing out.
pub const CAPACITY_HEADROOM_WEIGHT: f64 = 0.5;

pub fn score(
  reqs: &PropertyRequirements,
  controller_model: &ControllerModel,
  bom: &Bom,
  is_smart: bool,
) -> f64 {
  let mut s = BASE_SCORE;

  if let Some(budget) = reqs.budget_usd {
    if bom.total_usd <= budget {
      s += BUDGET_WITHIN_REWARD;
    } else {
      let overshoot = bom.total_usd - budget;
      s -= overshoot * BUDGET_OVER_PENALTY_PER_USD;
    }
  }

  if reqs.prefer_smart_controller && is_smart {
    s += SMART_PREFERENCE_REWARD;
  }

  let need = reqs.total_zone_count() as i64;
  let headroom = (controller_model.max_zones - need).max(0) as f64;
  s += headroom * CAPACITY_HEADROOM_WEIGHT;

  s
}
