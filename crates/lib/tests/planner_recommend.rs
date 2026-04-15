//! Integration tests for the catalog-driven planner.
//!
//! Drives `planner::recommend` against the real seeded catalog —
//! no mocks — so the tests catch any catalog change that would
//! break planning.

use std::path::PathBuf;

use automation_simulator_lib::catalog::Catalog;
use automation_simulator_lib::planner::{
  recommend, PlannerError, PropertyRequirements, YardRequirement,
  ZoneRequirement,
};
use automation_simulator_lib::sim::zone::PlantKind;

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
}

fn load_catalog() -> Catalog {
  Catalog::load(workspace_root().join("data").join("catalog"))
    .expect("seeded catalog loads")
}

fn small_reqs() -> PropertyRequirements {
  PropertyRequirements {
    property_id: "p1".into(),
    property_name: "Test Property".into(),
    climate_zone: "7b".into(),
    budget_usd: Some(1200.0),
    prefer_smart_controller: true,
    require_pressure_compensating: false,
    soil_type_id: "silty-clay-loam".into(),
    yards: vec![YardRequirement {
      id: "yard-a".into(),
      name: "Yard A".into(),
      area_sq_ft: 800.0,
      mains_pressure_psi: 60.0,
      zones: vec![
        ZoneRequirement {
          name_suffix: "veggies".into(),
          plant_kind: PlantKind::VeggieBed,
          area_sq_ft: 100.0,
        },
        ZoneRequirement {
          name_suffix: "shrubs".into(),
          plant_kind: PlantKind::Shrub,
          area_sq_ft: 200.0,
        },
      ],
    }],
  }
}

#[test]
fn recommends_multiple_plans_for_a_satisfiable_request() {
  let cat = load_catalog();
  let reqs = small_reqs();
  let plans = recommend(&reqs, &cat, 3).expect("planner returns plans");
  assert!(!plans.is_empty(), "expected at least one plan");
  assert!(plans.len() <= 3);
  for plan in &plans {
    assert!(plan.bom.total_usd > 0.0, "plan has a real BOM total");
    assert!(!plan.rationale.is_empty(), "plan has a rationale");
    assert!(
      plan.controller_max_zones >= reqs.total_zone_count() as i64,
      "picked controller covers requested zone count"
    );
  }
}

#[test]
fn plans_are_sorted_by_score_descending() {
  let cat = load_catalog();
  let reqs = small_reqs();
  let plans = recommend(&reqs, &cat, 5).expect("plans");
  for pair in plans.windows(2) {
    assert!(
      pair[0].score >= pair[1].score,
      "plans must be sorted high-to-low by score"
    );
  }
}

#[test]
fn recommending_is_deterministic() {
  let cat = load_catalog();
  let reqs = small_reqs();
  let a = recommend(&reqs, &cat, 3).expect("plans a");
  let b = recommend(&reqs, &cat, 3).expect("plans b");
  assert_eq!(a.len(), b.len());
  for (pa, pb) in a.iter().zip(b.iter()) {
    assert_eq!(pa.plan_id, pb.plan_id);
    assert_eq!(pa.controller_model_id, pb.controller_model_id);
    assert_eq!(pa.bom.total_usd, pb.bom.total_usd);
    assert_eq!(pa.score, pb.score);
  }
}

#[test]
fn zero_zones_returns_semantic_error() {
  let cat = load_catalog();
  let mut reqs = small_reqs();
  reqs.yards.iter_mut().for_each(|y| y.zones.clear());
  let err = recommend(&reqs, &cat, 3).unwrap_err();
  assert!(matches!(err, PlannerError::NoZonesRequested));
}

#[test]
fn unknown_soil_type_returns_semantic_error() {
  let cat = load_catalog();
  let mut reqs = small_reqs();
  reqs.soil_type_id = "moon-dust".into();
  let err = recommend(&reqs, &cat, 3).unwrap_err();
  assert!(matches!(err, PlannerError::UnknownSoilType { .. }));
}

#[test]
fn bom_includes_real_prices_and_pipe_and_hose() {
  let cat = load_catalog();
  let reqs = small_reqs();
  let plans = recommend(&reqs, &cat, 1).expect("plans");
  let plan = &plans[0];
  // Every plausible line category should show up: controller,
  // regulator, backflow, manifold, emitter, and mainline tubing.
  let categories: std::collections::BTreeSet<&str> =
    plan.bom.lines.iter().map(|l| l.category.as_str()).collect();
  for needed in [
    "controller",
    "pressure-regulator",
    "backflow-preventer",
    "manifold",
    "emitter",
    "mainline-tubing",
  ] {
    assert!(
      categories.contains(needed),
      "missing BOM category {needed:?}; got {categories:?}"
    );
  }
  // No line should be a zero-priced placeholder anymore.
  for line in &plan.bom.lines {
    assert!(
      line.unit_price_usd > 0.0,
      "line {:?} has a zero unit price",
      line.catalog_id
    );
    assert!(line.display_name != line.catalog_id || line.line_total_usd > 0.0);
  }
  // small_reqs has a shrub zone, which triggers branch tubing.
  assert!(
    categories.contains("branch-tubing"),
    "expected branch tubing for point-emitter zones"
  );
}

#[test]
fn hat_controllers_include_a_compute_host_line() {
  let cat = load_catalog();
  let reqs = small_reqs();
  // Ask the planner for enough candidates that at least one HAT
  // controller appears somewhere in the ranking.
  let plans = recommend(&reqs, &cat, 10).expect("plans");
  let mut saw_hat_plan_with_host = false;
  let mut saw_non_hat_plan_without_host = false;
  for plan in &plans {
    let model = cat
      .controllers
      .values()
      .find(|c| c.id.as_str() == plan.controller_model_id)
      .expect("plan controller in catalog");
    let has_host_line =
      plan.bom.lines.iter().any(|l| l.category == "compute-host");
    if model.requires_compute_host {
      assert!(
        has_host_line,
        "HAT controller {} missing a compute-host line",
        model.id
      );
      saw_hat_plan_with_host = true;
    } else {
      assert!(
        !has_host_line,
        "non-HAT controller {} wrongly got a compute-host line",
        model.id
      );
      saw_non_hat_plan_without_host = true;
    }
  }
  assert!(
    saw_hat_plan_with_host,
    "expected at least one plan whose controller requires a compute host"
  );
  assert!(
    saw_non_hat_plan_without_host,
    "expected at least one plan whose controller does not require a compute host"
  );
}

#[test]
fn smart_preference_actually_picks_a_smart_controller() {
  let cat = load_catalog();
  let reqs = small_reqs();
  let plans = recommend(&reqs, &cat, 5).expect("plans");
  let any_smart = plans.iter().any(|p| {
    cat
      .controllers
      .values()
      .find(|c| c.id.as_str() == p.controller_model_id)
      .map(|c| c.is_smart)
      .unwrap_or(false)
  });
  assert!(
    any_smart,
    "expected at least one candidate to be a smart controller"
  );
}
