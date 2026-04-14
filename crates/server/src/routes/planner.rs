//! Planner route: `POST /api/plan`.
//!
//! Takes a `PropertyRequirements` body, runs the catalog-driven
//! recommender, and returns a ranked list of candidate plans.
//! The server is stateless with respect to the planner — it reads
//! the catalog off `AppState` but never mutates anything.

use aide::axum::{routing::post_with, ApiRouter};
use aide::transform::TransformOperation;
use automation_simulator_lib::planner::{
  self, BomLine, PlannerError, PropertyPlan, PropertyRequirements,
  YardRequirement, ZoneRequirement,
};
use automation_simulator_lib::sim::zone::PlantKind;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{ApiError, ApiResult};
use crate::web_base::AppState;

// ── Request DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanRequest {
  pub property_id: String,
  pub property_name: String,
  pub climate_zone: String,
  pub yards: Vec<PlanYard>,
  #[serde(default)]
  pub budget_usd: Option<f64>,
  #[serde(default)]
  pub prefer_smart_controller: bool,
  #[serde(default)]
  pub require_pressure_compensating: bool,
  #[serde(default = "default_soil")]
  pub soil_type_id: String,
  /// How many ranked candidates to return.  Capped server-side so
  /// one huge request cannot blow up the response.
  #[serde(default = "default_top_n")]
  pub top_n: usize,
}

fn default_soil() -> String {
  "silty-clay-loam".into()
}
fn default_top_n() -> usize {
  3
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanYard {
  pub id: String,
  pub name: String,
  pub area_sq_ft: f64,
  pub mains_pressure_psi: f64,
  pub zones: Vec<PlanZone>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanZone {
  pub name_suffix: String,
  pub plant_kind: String,
  pub area_sq_ft: f64,
}

// ── Response DTOs ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanResponse {
  pub plans: Vec<PlanDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanDto {
  pub plan_id: String,
  pub controller_model_id: String,
  pub controller_max_zones: i64,
  pub score: f64,
  pub rationale: Vec<String>,
  pub bom: BomDto,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BomDto {
  pub lines: Vec<BomLineDto>,
  pub total_usd: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BomLineDto {
  pub category: String,
  pub catalog_id: String,
  pub display_name: String,
  pub manufacturer: String,
  pub quantity: i64,
  pub unit_price_usd: f64,
  pub line_total_usd: f64,
}

impl From<&PropertyPlan> for PlanDto {
  fn from(p: &PropertyPlan) -> Self {
    Self {
      plan_id: p.plan_id.clone(),
      controller_model_id: p.controller_model_id.clone(),
      controller_max_zones: p.controller_max_zones,
      score: p.score,
      rationale: p.rationale.clone(),
      bom: BomDto {
        lines: p.bom.lines.iter().map(BomLineDto::from).collect(),
        total_usd: p.bom.total_usd,
      },
    }
  }
}

impl From<&BomLine> for BomLineDto {
  fn from(l: &BomLine) -> Self {
    Self {
      category: l.category.clone(),
      catalog_id: l.catalog_id.clone(),
      display_name: l.display_name.clone(),
      manufacturer: l.manufacturer.clone(),
      quantity: l.quantity,
      unit_price_usd: l.unit_price_usd,
      line_total_usd: l.line_total_usd,
    }
  }
}

// ── Mapping helpers ─────────────────────────────────────────────────────────

fn plant_kind_from_kebab(s: &str) -> Result<PlantKind, ApiError> {
  match s {
    "veggie-bed" => Ok(PlantKind::VeggieBed),
    "shrub" => Ok(PlantKind::Shrub),
    "perennial" => Ok(PlantKind::Perennial),
    "tree" => Ok(PlantKind::Tree),
    other => Err(ApiError::BadRequest(format!(
      "unknown plant_kind {other:?}; expected one of: veggie-bed, shrub, perennial, tree"
    ))),
  }
}

fn to_requirements(req: PlanRequest) -> Result<PropertyRequirements, ApiError> {
  let mut yards = Vec::with_capacity(req.yards.len());
  for y in req.yards {
    let mut zones = Vec::with_capacity(y.zones.len());
    for z in y.zones {
      zones.push(ZoneRequirement {
        name_suffix: z.name_suffix,
        plant_kind: plant_kind_from_kebab(&z.plant_kind)?,
        area_sq_ft: z.area_sq_ft,
      });
    }
    yards.push(YardRequirement {
      id: y.id,
      name: y.name,
      area_sq_ft: y.area_sq_ft,
      mains_pressure_psi: y.mains_pressure_psi,
      zones,
    });
  }
  Ok(PropertyRequirements {
    property_id: req.property_id,
    property_name: req.property_name,
    climate_zone: req.climate_zone,
    yards,
    budget_usd: req.budget_usd,
    prefer_smart_controller: req.prefer_smart_controller,
    require_pressure_compensating: req.require_pressure_compensating,
    soil_type_id: req.soil_type_id,
  })
}

fn planner_error_to_api(e: PlannerError) -> ApiError {
  use PlannerError::*;
  match e {
    NoZonesRequested
    | NoControllerLargeEnough { .. }
    | NoManifoldLargeEnough { .. }
    | NoEmitterForPlantKind { .. }
    | NoPressureRegulator { .. }
    | UnknownSoilType { .. } => ApiError::BadRequest(e.to_string()),
    NoBackflowPreventer | PlanInternallyInvalid(_) => {
      ApiError::Internal(e.to_string())
    }
  }
}

// ── Handler ─────────────────────────────────────────────────────────────────

async fn plan(
  State(state): State<AppState>,
  Json(req): Json<PlanRequest>,
) -> ApiResult<PlanResponse> {
  let top_n = req.top_n.clamp(1, 10);
  let reqs = to_requirements(req).map_err(|e| e.into_response())?;
  let plans = planner::recommend(&reqs, &state.catalog, top_n)
    .map_err(|e| planner_error_to_api(e).into_response())?;
  let dtos: Vec<PlanDto> = plans.iter().map(PlanDto::from).collect();
  Ok(Json(PlanResponse { plans: dtos }))
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new().api_route(
    "/api/plan",
    post_with(plan, |op: TransformOperation| {
      op.description(
        "Given a PropertyRequirements, return a ranked list of \
         candidate irrigation plans with full BOMs and rationales. \
         Deterministic; catalog-driven.",
      )
    }),
  )
}
