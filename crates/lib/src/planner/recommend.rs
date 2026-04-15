//! Deterministic recommender.
//!
//! Given a `PropertyRequirements` and the loaded `Catalog`, the
//! recommender enumerates plausible hardware combinations and
//! returns the top-N candidates ranked by [`score`].  Determinism
//! is critical: same inputs ⇒ same ranked output, every time.
//!
//! Strategy is intentionally simple for v0.1:
//!
//! 1. Filter controllers that can handle the total zone count.
//!    Pick three: cheapest, smart-preferred (if any), most-capable.
//! 2. For each yard, pick a manifold with `zone_capacity` ≥
//!    that yard's zone count.  Cheapest fit.
//! 3. For each zone, pick an emitter that matches the plant kind's
//!    typical pairings (filtered by pressure-compensating if
//!    requested).  Cheapest fit per kind.
//! 4. Pick a 25-PSI-output regulator that accepts the yard's
//!    mains pressure.
//! 5. Pick the cheapest backflow preventer.
//! 6. Compose into a `PropertyBundle`, validate, score, return.

use crate::catalog::{
  BackflowPreventerModel, Catalog, EmitterShape, EmitterSpec, ManifoldModel,
  PressureRegulatorModel,
};
use crate::seed::PropertyBundle;
use crate::sim::hardware::{ControllerInstance, ControllerInstanceRaw};
use crate::sim::id::{
  ControllerInstanceId, ControllerModelId, EmitterSpecId, ManifoldInstanceId,
  ManifoldModelId, PropertyId, SoilTypeId, SpigotId, YardId, ZoneId,
};
use crate::sim::property::{Property, Spigot, Yard};
use crate::sim::zone::{Manifold, PlantKind, Zone};

use super::errors::PlannerError;
use super::plan::{Bom, BomLine, PropertyPlan};
use super::requirements::{PropertyRequirements, YardRequirement};
use super::scoring::score;

/// Top-level entry point.  Returns up to `top_n` plans ranked by
/// score (descending).  Returns at least one plan when any valid
/// configuration exists; an empty vector means the requirements
/// are unsatisfiable in a way captured by the returned error.
pub fn recommend(
  reqs: &PropertyRequirements,
  catalog: &Catalog,
  top_n: usize,
) -> Result<Vec<PropertyPlan>, PlannerError> {
  if reqs.total_zone_count() == 0 {
    return Err(PlannerError::NoZonesRequested);
  }
  if !catalog
    .soil_types
    .contains_key(&SoilTypeId::new(&*reqs.soil_type_id))
  {
    return Err(PlannerError::UnknownSoilType {
      soil_type: reqs.soil_type_id.clone(),
    });
  }

  let controllers = pick_controllers(reqs, catalog)?;
  let mut plans: Vec<PropertyPlan> = Vec::with_capacity(controllers.len());
  for (idx, ctrl_id) in controllers.iter().enumerate() {
    let plan_id = format!("{}-plan-{}", reqs.property_id, idx + 1);
    plans.push(build_plan(reqs, catalog, ctrl_id, plan_id)?);
  }

  // Sort by score descending; stable tiebreak by plan_id keeps
  // output deterministic.
  plans.sort_by(|a, b| {
    b.score
      .partial_cmp(&a.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| a.plan_id.cmp(&b.plan_id))
  });
  plans.truncate(top_n);
  Ok(plans)
}

/// Up-to-three controllers covering the request: cheapest, smart-
/// preferred (if applicable + distinct), most-capable.  "Smart" is
/// a per-row flag on `ControllerModel` so the catalog is the
/// single source of truth — the planner itself is brand-agnostic.
fn pick_controllers(
  reqs: &PropertyRequirements,
  catalog: &Catalog,
) -> Result<Vec<ControllerModelId>, PlannerError> {
  let total_zones = reqs.total_zone_count() as i64;
  let max_avail = catalog
    .controllers
    .values()
    .map(|c| c.max_zones)
    .max()
    .unwrap_or(0);
  if total_zones > max_avail {
    return Err(PlannerError::NoControllerLargeEnough {
      requested: reqs.total_zone_count(),
      max_available: max_avail,
    });
  }

  let mut viable: Vec<&crate::catalog::ControllerModel> = catalog
    .controllers
    .values()
    .filter(|c| c.max_zones >= total_zones)
    .collect();
  // Stable order by id so the ranking is deterministic.
  viable.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

  let cheapest = viable
    .iter()
    .min_by(|a, b| {
      a.price_usd
        .partial_cmp(&b.price_usd)
        .unwrap_or(std::cmp::Ordering::Equal)
    })
    .map(|c| c.id.clone());

  let smart = if reqs.prefer_smart_controller {
    viable
      .iter()
      .filter(|c| c.is_smart)
      .min_by(|a, b| {
        a.price_usd
          .partial_cmp(&b.price_usd)
          .unwrap_or(std::cmp::Ordering::Equal)
      })
      .map(|c| c.id.clone())
  } else {
    None
  };

  let most_capable = viable
    .iter()
    .max_by_key(|c| c.max_zones)
    .map(|c| c.id.clone());

  let mut out: Vec<ControllerModelId> = Vec::with_capacity(3);
  for choice in [cheapest, smart, most_capable].into_iter().flatten() {
    if !out.contains(&choice) {
      out.push(choice);
    }
  }
  if out.is_empty() {
    return Err(PlannerError::NoControllerLargeEnough {
      requested: reqs.total_zone_count(),
      max_available: max_avail,
    });
  }
  Ok(out)
}

fn build_plan(
  reqs: &PropertyRequirements,
  catalog: &Catalog,
  controller_id: &ControllerModelId,
  plan_id: String,
) -> Result<PropertyPlan, PlannerError> {
  let controller_model = catalog
    .controllers
    .get(controller_id)
    .expect("controller existed");

  // Yards + spigots straight from requirements.
  let yards: Vec<Yard> = reqs
    .yards
    .iter()
    .map(|y| Yard {
      id: YardId::new(&y.id),
      name: y.name.clone(),
      area_sq_ft: y.area_sq_ft,
    })
    .collect();
  let spigots: Vec<Spigot> = reqs
    .yards
    .iter()
    .map(|y| Spigot {
      id: SpigotId::new(format!("spigot-{}", y.id)),
      mains_pressure_psi: y.mains_pressure_psi,
      notes: None,
    })
    .collect();

  // One manifold per yard.
  let mut manifolds: Vec<Manifold> = Vec::with_capacity(reqs.yards.len());
  let mut zones: Vec<Zone> = Vec::new();
  let soil_id = SoilTypeId::new(&*reqs.soil_type_id);

  for yard in &reqs.yards {
    let manifold_model = pick_manifold(yard, catalog)?;
    let manifold_instance_id =
      ManifoldInstanceId::new(format!("manifold-{}", yard.id));
    manifolds.push(Manifold {
      id: manifold_instance_id.clone(),
      model_id: manifold_model.id.clone(),
      spigot_id: SpigotId::new(format!("spigot-{}", yard.id)),
      zone_capacity: manifold_model.zone_capacity,
    });

    for zone_req in &yard.zones {
      let emitter = pick_emitter(
        zone_req.plant_kind,
        reqs.require_pressure_compensating,
        catalog,
      )?;
      zones.push(Zone {
        id: ZoneId::new(format!("{}-{}", yard.id, zone_req.name_suffix)),
        yard_id: YardId::new(&yard.id),
        manifold_id: manifold_instance_id.clone(),
        plant_kind: zone_req.plant_kind,
        emitter_spec_id: emitter.id.clone(),
        soil_type_id: soil_id.clone(),
        area_sq_ft: zone_req.area_sq_ft,
        notes: None,
      });
    }
  }

  // One controller wired to every zone, in zone order.
  let controllers_inst: Vec<ControllerInstance> =
    vec![ControllerInstance::try_from_raw(ControllerInstanceRaw {
      id: ControllerInstanceId::new("controller-main"),
      model_id: controller_model.id.clone(),
      zone_assignments: zones.iter().map(|z| z.id.clone()).collect(),
      notes: None,
    })
    .expect("controller validation succeeds at build time")];

  let property = Property {
    id: PropertyId::new(&*reqs.property_id),
    name: reqs.property_name.clone(),
    lot_area_sq_ft: yards.iter().map(|y| y.area_sq_ft).sum::<f64>().max(1.0),
    climate_zone: reqs.climate_zone.clone(),
    yards,
    spigots,
  };

  let bundle = PropertyBundle {
    property,
    manifolds,
    zones,
    plants: Vec::new(),
    controllers: controllers_inst,
    sensors: Vec::new(),
    weather_stations: Vec::new(),
    schedule: Vec::new(),
  };

  // Compose the BOM out of the picks above plus a 25-PSI
  // regulator and a basic backflow preventer per spigot.
  let regulator = pick_regulator(catalog, &reqs.yards)?;
  let backflow = pick_backflow(catalog)?;
  let bom =
    compose_bom(reqs, &bundle, catalog, controller_model, regulator, backflow);

  let s = score(reqs, controller_model, &bom, controller_model.is_smart);
  let rationale = build_rationale(controller_model, &bom, reqs);

  Ok(PropertyPlan {
    plan_id,
    bundle,
    bom,
    score: s,
    rationale,
    controller_model_id: controller_model.id.as_str().to_string(),
    controller_max_zones: controller_model.max_zones,
  })
}

fn pick_manifold<'a>(
  yard: &YardRequirement,
  catalog: &'a Catalog,
) -> Result<&'a ManifoldModel, PlannerError> {
  let need = yard.zones.len() as i64;
  let max_avail = catalog
    .manifolds
    .values()
    .map(|m| m.zone_capacity)
    .max()
    .unwrap_or(0);
  let mut viable: Vec<&ManifoldModel> = catalog
    .manifolds
    .values()
    .filter(|m| m.zone_capacity >= need)
    .collect();
  if viable.is_empty() {
    return Err(PlannerError::NoManifoldLargeEnough {
      yard: yard.id.clone(),
      requested: yard.zones.len(),
      max_available: max_avail,
    });
  }
  viable.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
  let pick = viable.iter().min_by(|a, b| {
    a.price_usd
      .partial_cmp(&b.price_usd)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  Ok(*pick.expect("non-empty after filter"))
}

fn pick_emitter<'a>(
  plant_kind: PlantKind,
  pc_required: bool,
  catalog: &'a Catalog,
) -> Result<&'a EmitterSpec, PlannerError> {
  let preferred_shape = match plant_kind {
    PlantKind::VeggieBed => EmitterShape::InlineDrip,
    PlantKind::Shrub | PlantKind::Perennial => EmitterShape::PointEmitter,
    PlantKind::Tree => EmitterShape::Bubbler,
  };
  let mut viable: Vec<&EmitterSpec> = catalog
    .emitters
    .values()
    .filter(|e| e.shape == preferred_shape)
    .filter(|e| !pc_required || e.pressure_compensating)
    .collect();
  if viable.is_empty() {
    // Fall back to point emitters of the right pressure-comp class
    // for any plant kind, so we never hard-fail just because the
    // user picked an unusual plant kind.
    viable = catalog
      .emitters
      .values()
      .filter(|e| !pc_required || e.pressure_compensating)
      .collect();
  }
  if viable.is_empty() {
    return Err(PlannerError::NoEmitterForPlantKind {
      plant_kind,
      pc_required,
    });
  }
  viable.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
  let pick = viable.iter().min_by(|a, b| {
    a.price_usd_per_100
      .partial_cmp(&b.price_usd_per_100)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  Ok(*pick.expect("non-empty"))
}

fn pick_regulator<'a>(
  catalog: &'a Catalog,
  yards: &[YardRequirement],
) -> Result<&'a PressureRegulatorModel, PlannerError> {
  let max_mains = yards
    .iter()
    .map(|y| y.mains_pressure_psi)
    .fold(0.0_f64, f64::max);
  let mut viable: Vec<&PressureRegulatorModel> = catalog
    .pressure_regulators
    .values()
    .filter(|r| r.input_psi_min <= max_mains && max_mains <= r.input_psi_max)
    .collect();
  if viable.is_empty() {
    return Err(PlannerError::NoPressureRegulator {
      mains_psi: max_mains,
    });
  }
  viable.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
  let pick = viable.iter().min_by(|a, b| {
    a.price_usd
      .partial_cmp(&b.price_usd)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  Ok(*pick.expect("non-empty"))
}

fn pick_backflow(
  catalog: &Catalog,
) -> Result<&BackflowPreventerModel, PlannerError> {
  if catalog.backflow_preventers.is_empty() {
    return Err(PlannerError::NoBackflowPreventer);
  }
  let mut viable: Vec<&BackflowPreventerModel> =
    catalog.backflow_preventers.values().collect();
  viable.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
  let pick = viable.iter().min_by(|a, b| {
    a.price_usd
      .partial_cmp(&b.price_usd)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  Ok(*pick.expect("non-empty"))
}

/// Outer-diameter cutoff (inches) that splits the catalog's
/// `drip-lines` rows into "mainline" (≥ cutoff, bigger tubing that
/// carries water from the regulator to the manifold and out to
/// each zone) and "branch" (< cutoff, skinnier tubing that runs
/// off the mainline to individual point emitters).  0.4" covers
/// the 1/2"-vs-1/4" distinction with room to spare for fractional
/// sizes the catalog might grow later.
const MAINLINE_MIN_OD_INCHES: f64 = 0.4;

/// Rough mainline-feet-per-square-foot heuristic — a v0.3
/// approximation good enough for budget comparisons.  Real
/// layouts vary by plant spacing and bed geometry.
const MAINLINE_FEET_PER_ZONE_SQ_FT: f64 = 0.10;

/// Per-yard mainline allowance: roughly the run from the spigot /
/// backflow / regulator back to the manifold.  Flat constant in
/// v0.3; a follow-up can infer it from yard area.
const MAINLINE_FEET_PER_YARD: f64 = 20.0;

/// Feet of 1/4" branch tubing per individual point emitter — short
/// runs from the mainline to the plant.
const BRANCH_FEET_PER_POINT_EMITTER: f64 = 2.0;

fn compose_bom(
  reqs: &PropertyRequirements,
  bundle: &PropertyBundle,
  catalog: &Catalog,
  controller_model: &crate::catalog::ControllerModel,
  regulator: &PressureRegulatorModel,
  backflow: &BackflowPreventerModel,
) -> Bom {
  let mut lines: Vec<BomLine> = Vec::new();

  lines.push(BomLine {
    category: "controller".into(),
    catalog_id: controller_model.id.as_str().to_string(),
    display_name: controller_model.name.clone(),
    manufacturer: controller_model.manufacturer.clone(),
    quantity: 1,
    unit_price_usd: controller_model.price_usd,
    line_total_usd: controller_model.price_usd,
  });

  // Compute host: controllers that mount as a Pi HAT cannot run
  // on their own.  Add the cheapest compatible SBC to the BOM so
  // the plan is actually buildable.  Controllers with
  // `requires_compute_host = false` (the default, which covers
  // every controller that ships with built-in compute) skip this
  // entirely.
  if controller_model.requires_compute_host {
    if let Some(host) = pick_compute_host(catalog) {
      lines.push(BomLine {
        category: "compute-host".into(),
        catalog_id: host.id.as_str().to_string(),
        display_name: host.name.clone(),
        manufacturer: host.manufacturer.clone(),
        quantity: 1,
        unit_price_usd: host.price_usd,
        line_total_usd: host.price_usd,
      });
    }
  }

  // One regulator + one backflow per spigot (one per yard).
  let yard_count = reqs.yards.len() as i64;
  lines.push(BomLine {
    category: "pressure-regulator".into(),
    catalog_id: regulator.id.as_str().to_string(),
    display_name: regulator.name.clone(),
    manufacturer: regulator.manufacturer.clone(),
    quantity: yard_count,
    unit_price_usd: regulator.price_usd,
    line_total_usd: regulator.price_usd * yard_count as f64,
  });
  lines.push(BomLine {
    category: "backflow-preventer".into(),
    catalog_id: backflow.id.as_str().to_string(),
    display_name: backflow.name.clone(),
    manufacturer: backflow.manufacturer.clone(),
    quantity: yard_count,
    unit_price_usd: backflow.price_usd,
    line_total_usd: backflow.price_usd * yard_count as f64,
  });

  // Manifolds — dedupe by model id; look prices + names up in the
  // catalog so each line carries real numbers a reader can verify.
  let mut manifold_counts: std::collections::BTreeMap<String, i64> =
    Default::default();
  for m in &bundle.manifolds {
    *manifold_counts
      .entry(m.model_id.as_str().to_string())
      .or_insert(0) += 1;
  }
  for (model_id, count) in manifold_counts {
    if let Some(model) =
      catalog.manifolds.get(&ManifoldModelId::new(&*model_id))
    {
      lines.push(BomLine {
        category: "manifold".into(),
        catalog_id: model_id,
        display_name: model.name.clone(),
        manufacturer: model.manufacturer.clone(),
        quantity: count,
        unit_price_usd: model.price_usd,
        line_total_usd: model.price_usd * count as f64,
      });
    }
  }

  // Emitters — dedupe by emitter-spec id; estimated quantity per
  // zone follows the shape of the emitter (inline drip counts ~1
  // per sq ft at 12" spacing; point emitters count heads per
  // plant).  Pricing in the catalog is per-100, so the unit price
  // shown on the BOM is the per-100 divided by 100.
  let mut emitter_counts: std::collections::BTreeMap<String, i64> =
    Default::default();
  for z in &bundle.zones {
    let est = match z.plant_kind {
      PlantKind::VeggieBed => z.area_sq_ft as i64, // ~1 emitter per sq ft at 12" spacing
      PlantKind::Shrub => (z.area_sq_ft / 10.0).ceil() as i64,
      PlantKind::Perennial => (z.area_sq_ft / 5.0).ceil() as i64,
      PlantKind::Tree => (z.area_sq_ft / 20.0).ceil() as i64,
    };
    *emitter_counts
      .entry(z.emitter_spec_id.as_str().to_string())
      .or_insert(0) += est.max(1);
  }
  for (eid, count) in emitter_counts {
    if let Some(spec) = catalog.emitters.get(&EmitterSpecId::new(&*eid)) {
      let unit = spec.price_usd_per_100 / 100.0;
      lines.push(BomLine {
        category: "emitter".into(),
        catalog_id: eid,
        display_name: spec.name.clone(),
        manufacturer: spec.manufacturer.clone(),
        quantity: count,
        unit_price_usd: unit,
        line_total_usd: unit * count as f64,
      });
    }
  }

  // Tubing.  Mainline (1/2" poly) carries water from the regulator
  // out to every zone — the catalog-driven BOM isn't complete
  // without it.  Branch tubing (1/4") only matters when there are
  // point emitters to feed; inline drip beds use the emitter
  // tubing itself as the distribution line.
  let mainline_feet: f64 = (MAINLINE_FEET_PER_YARD * yard_count as f64)
    + bundle
      .zones
      .iter()
      .map(|z| (z.area_sq_ft * MAINLINE_FEET_PER_ZONE_SQ_FT).max(5.0))
      .sum::<f64>();
  let branch_feet: f64 = bundle
    .zones
    .iter()
    .filter(|z| !matches!(z.plant_kind, PlantKind::VeggieBed))
    .map(|z| {
      let heads = match z.plant_kind {
        PlantKind::Shrub => (z.area_sq_ft / 10.0).ceil(),
        PlantKind::Perennial => (z.area_sq_ft / 5.0).ceil(),
        PlantKind::Tree => (z.area_sq_ft / 20.0).ceil(),
        PlantKind::VeggieBed => 0.0,
      };
      heads * BRANCH_FEET_PER_POINT_EMITTER
    })
    .sum();

  if let Some(mainline) = pick_drip_line(catalog, /* want_mainline */ true) {
    let qty = mainline_feet.ceil() as i64;
    lines.push(BomLine {
      category: "mainline-tubing".into(),
      catalog_id: mainline.id.as_str().to_string(),
      display_name: mainline.name.clone(),
      manufacturer: mainline.manufacturer.clone(),
      quantity: qty,
      unit_price_usd: mainline.price_usd_per_foot,
      line_total_usd: mainline.price_usd_per_foot * qty as f64,
    });
  }
  if branch_feet > 0.0 {
    if let Some(branch) =
      pick_drip_line(catalog, /* want_mainline */ false)
    {
      let qty = branch_feet.ceil() as i64;
      lines.push(BomLine {
        category: "branch-tubing".into(),
        catalog_id: branch.id.as_str().to_string(),
        display_name: branch.name.clone(),
        manufacturer: branch.manufacturer.clone(),
        quantity: qty,
        unit_price_usd: branch.price_usd_per_foot,
        line_total_usd: branch.price_usd_per_foot * qty as f64,
      });
    }
  }

  Bom::from_lines(lines)
}

/// Cheapest SBC that accepts a Raspberry Pi HAT, since that is the
/// form factor we care about in v0.3.  Returns `None` when the
/// catalog has no compatible row — the planner then drops the
/// compute-host line rather than hard-failing, matching how
/// missing drip lines degrade.
fn pick_compute_host(
  catalog: &Catalog,
) -> Option<&crate::catalog::ComputeHostModel> {
  let mut viable: Vec<&crate::catalog::ComputeHostModel> = catalog
    .compute_hosts
    .values()
    .filter(|h| h.supports_raspberry_pi_hat)
    .collect();
  viable.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
  viable.into_iter().min_by(|a, b| {
    a.price_usd
      .partial_cmp(&b.price_usd)
      .unwrap_or(std::cmp::Ordering::Equal)
  })
}

fn pick_drip_line(
  catalog: &Catalog,
  want_mainline: bool,
) -> Option<&crate::catalog::DripLineModel> {
  let mut viable: Vec<&crate::catalog::DripLineModel> = catalog
    .drip_lines
    .values()
    .filter(|d| {
      if want_mainline {
        d.outer_diameter_inches >= MAINLINE_MIN_OD_INCHES
      } else {
        d.outer_diameter_inches < MAINLINE_MIN_OD_INCHES
      }
    })
    .collect();
  viable.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
  viable.into_iter().min_by(|a, b| {
    a.price_usd_per_foot
      .partial_cmp(&b.price_usd_per_foot)
      .unwrap_or(std::cmp::Ordering::Equal)
  })
}

fn build_rationale(
  controller_model: &crate::catalog::ControllerModel,
  bom: &Bom,
  reqs: &PropertyRequirements,
) -> Vec<String> {
  let mut out = Vec::new();
  out.push(format!(
    "Controller: {} ({}; up to {} zones; ${:.2}).",
    controller_model.name,
    controller_model.manufacturer,
    controller_model.max_zones,
    controller_model.price_usd
  ));
  out.push(format!("Total estimated hardware cost: ${:.2}.", bom.total_usd));
  if let Some(budget) = reqs.budget_usd {
    if bom.total_usd > budget {
      out.push(format!(
        "Over budget (${:.2} > ${:.2}); ranking penalised.",
        bom.total_usd, budget
      ));
    } else {
      out.push(format!(
        "Within budget (${:.2} ≤ ${:.2}).",
        bom.total_usd, budget
      ));
    }
  }
  if reqs.prefer_smart_controller && controller_model.is_smart {
    out.push("Smart controller (Wi-Fi + app); preference satisfied.".into());
  }
  out
}
