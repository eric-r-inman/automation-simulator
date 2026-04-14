//! Zone definition CRUD routes.
//!
//! These endpoints add / edit / remove zones from the running
//! property + simulator world.  Operational endpoints (run / stop)
//! stay in `routes::zones`; this module holds only the changes to
//! the *definition* of a zone.
//!
//! Lifecycle for each mutation:
//!
//! 1. Validate the request body against the in-memory catalog and
//!    the property's yards / manifolds.  Reject early with a typed
//!    `ApiError` so the client can react before any state changes.
//! 2. Lock the property bundle, lock the simulator world; mutate
//!    both atomically; drop both locks; respond.
//!
//! v0.1 keeps these mutations in memory only; restarting the
//! server reloads the property TOML.  A follow-up will persist
//! changes to SQLite so they survive restart.

use aide::axum::{
  routing::{get_with, post_with},
  ApiRouter,
};
use aide::transform::TransformOperation;
use automation_simulator_lib::sim::id::{
  EmitterSpecId, ManifoldInstanceId, SoilTypeId, YardId, ZoneId,
};
use automation_simulator_lib::sim::zone::{PlantKind, Zone};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::{api_err, ApiError, ApiResult};
use crate::web_base::AppState;

// ── DTOs ────────────────────────────────────────────────────────────────────

/// Path-params wrapper so aide can document the `:zone_id` segment.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ZoneIdPath {
  pub zone_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ZoneCreate {
  pub id: String,
  pub yard_id: String,
  pub manifold_id: String,
  pub plant_kind: String,
  pub emitter_spec_id: String,
  pub soil_type_id: String,
  pub area_sq_ft: f64,
  #[serde(default)]
  pub notes: Option<String>,
  /// Initial soil moisture (volumetric water content, 0..1).
  /// Defaults to 0.30 — the same value the boot path uses.
  #[serde(default)]
  pub initial_vwc: Option<f64>,
}

/// Partial-update body.  Every field is optional; absent fields
/// keep their existing value.  The zone id itself is the URL
/// parameter and cannot be changed (delete + create instead).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ZoneUpdate {
  #[serde(default)]
  pub yard_id: Option<String>,
  #[serde(default)]
  pub manifold_id: Option<String>,
  #[serde(default)]
  pub plant_kind: Option<String>,
  #[serde(default)]
  pub emitter_spec_id: Option<String>,
  #[serde(default)]
  pub soil_type_id: Option<String>,
  #[serde(default)]
  pub area_sq_ft: Option<f64>,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ZoneDefinition {
  pub id: String,
  pub yard_id: String,
  pub manifold_id: String,
  pub plant_kind: String,
  pub emitter_spec_id: String,
  pub soil_type_id: String,
  pub area_sq_ft: f64,
  pub notes: Option<String>,
}

impl From<&Zone> for ZoneDefinition {
  fn from(z: &Zone) -> Self {
    Self {
      id: z.id.as_str().to_string(),
      yard_id: z.yard_id.as_str().to_string(),
      manifold_id: z.manifold_id.as_str().to_string(),
      plant_kind: plant_kind_to_kebab(z.plant_kind),
      emitter_spec_id: z.emitter_spec_id.as_str().to_string(),
      soil_type_id: z.soil_type_id.as_str().to_string(),
      area_sq_ft: z.area_sq_ft,
      notes: z.notes.clone(),
    }
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeletedResponse {
  pub zone_id: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

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

fn plant_kind_to_kebab(k: PlantKind) -> String {
  match k {
    PlantKind::VeggieBed => "veggie-bed",
    PlantKind::Shrub => "shrub",
    PlantKind::Perennial => "perennial",
    PlantKind::Tree => "tree",
  }
  .to_string()
}

// ── Handlers ────────────────────────────────────────────────────────────────

async fn get_zone_definition(
  State(state): State<AppState>,
  Path(path): Path<ZoneIdPath>,
) -> ApiResult<ZoneDefinition> {
  let bundle = state.property.lock().await;
  let zone = bundle
    .zones
    .iter()
    .find(|z| z.id.as_str() == path.zone_id)
    .ok_or_else(|| {
      ApiError::ZoneNotFound(format!("zone {}", path.zone_id)).into_response()
    })?;
  Ok(Json(ZoneDefinition::from(zone)))
}

async fn create_zone(
  State(state): State<AppState>,
  Json(req): Json<ZoneCreate>,
) -> ApiResult<ZoneDefinition> {
  if req.id.trim().is_empty() {
    return Err(
      ApiError::BadRequest("zone id must not be blank".into()).into_response(),
    );
  }
  if !(req.area_sq_ft > 0.0) {
    return Err(
      ApiError::BadRequest("area_sq_ft must be > 0".into()).into_response(),
    );
  }
  let plant_kind =
    plant_kind_from_kebab(&req.plant_kind).map_err(|e| e.into_response())?;

  // Cross-references against the property's existing entities.
  // Done under the property lock to avoid a race with concurrent
  // mutations.
  let mut bundle = state.property.lock().await;
  if bundle.zones.iter().any(|z| z.id.as_str() == req.id) {
    return Err(
      ApiError::BadRequest(format!(
        "zone {} already exists in the property",
        req.id
      ))
      .into_response(),
    );
  }
  if !bundle
    .property
    .yards
    .iter()
    .any(|y| y.id.as_str() == req.yard_id)
  {
    return Err(
      ApiError::BadRequest(format!(
        "yard_id {} does not exist on this property",
        req.yard_id
      ))
      .into_response(),
    );
  }
  if !bundle
    .manifolds
    .iter()
    .any(|m| m.id.as_str() == req.manifold_id)
  {
    return Err(
      ApiError::BadRequest(format!(
        "manifold_id {} does not exist on this property",
        req.manifold_id
      ))
      .into_response(),
    );
  }

  // Catalog references.
  if !state
    .catalog
    .soil_types
    .contains_key(&SoilTypeId::new(&*req.soil_type_id))
  {
    return Err(
      ApiError::BadRequest(format!(
        "soil_type_id {} is not in the catalog",
        req.soil_type_id
      ))
      .into_response(),
    );
  }
  if !state
    .catalog
    .emitters
    .contains_key(&EmitterSpecId::new(&*req.emitter_spec_id))
  {
    return Err(
      ApiError::BadRequest(format!(
        "emitter_spec_id {} is not in the catalog",
        req.emitter_spec_id
      ))
      .into_response(),
    );
  }

  let zone = Zone {
    id: ZoneId::new(req.id.clone()),
    yard_id: YardId::new(req.yard_id),
    manifold_id: ManifoldInstanceId::new(req.manifold_id),
    plant_kind,
    emitter_spec_id: EmitterSpecId::new(req.emitter_spec_id),
    soil_type_id: SoilTypeId::new(req.soil_type_id),
    area_sq_ft: req.area_sq_ft,
    notes: req.notes,
  };

  // Push into both the in-memory PropertyBundle and the running
  // SimWorld atomically.  Errors from SimWorld::add_zone (e.g.
  // catalog ref drift) roll back the property mutation by simply
  // not having performed it yet.
  let initial_vwc = req.initial_vwc.unwrap_or(0.30);
  {
    let mut world = state.world.0.lock().await;
    world.add_zone(zone.clone(), initial_vwc).map_err(|e| {
      ApiError::Internal(format!("SimWorld add_zone failed: {e}"))
        .into_response()
    })?;
  }
  bundle.zones.push(zone.clone());
  info!(zone_id = %zone.id, "Created zone");
  Ok(Json(ZoneDefinition::from(&zone)))
}

async fn update_zone(
  State(state): State<AppState>,
  Path(path): Path<ZoneIdPath>,
  Json(req): Json<ZoneUpdate>,
) -> ApiResult<ZoneDefinition> {
  let mut bundle = state.property.lock().await;
  let idx = bundle
    .zones
    .iter()
    .position(|z| z.id.as_str() == path.zone_id)
    .ok_or_else(|| {
      ApiError::ZoneNotFound(format!("zone {}", path.zone_id)).into_response()
    })?;

  // Build the merged zone outside the mutable borrow scope.
  let mut merged = bundle.zones[idx].clone();
  if let Some(yard_id) = req.yard_id {
    if !bundle
      .property
      .yards
      .iter()
      .any(|y| y.id.as_str() == yard_id)
    {
      return Err(
        ApiError::BadRequest(format!("yard_id {yard_id} not on property"))
          .into_response(),
      );
    }
    merged.yard_id = YardId::new(yard_id);
  }
  if let Some(manifold_id) = req.manifold_id {
    if !bundle
      .manifolds
      .iter()
      .any(|m| m.id.as_str() == manifold_id)
    {
      return Err(
        ApiError::BadRequest(format!(
          "manifold_id {manifold_id} not on property"
        ))
        .into_response(),
      );
    }
    merged.manifold_id = ManifoldInstanceId::new(manifold_id);
  }
  if let Some(pk) = req.plant_kind {
    merged.plant_kind =
      plant_kind_from_kebab(&pk).map_err(|e| e.into_response())?;
  }
  if let Some(es) = req.emitter_spec_id {
    if !state
      .catalog
      .emitters
      .contains_key(&EmitterSpecId::new(&*es))
    {
      return Err(
        ApiError::BadRequest(format!(
          "emitter_spec_id {es} is not in the catalog"
        ))
        .into_response(),
      );
    }
    merged.emitter_spec_id = EmitterSpecId::new(es);
  }
  if let Some(st) = req.soil_type_id {
    if !state
      .catalog
      .soil_types
      .contains_key(&SoilTypeId::new(&*st))
    {
      return Err(
        ApiError::BadRequest(format!(
          "soil_type_id {st} is not in the catalog"
        ))
        .into_response(),
      );
    }
    merged.soil_type_id = SoilTypeId::new(st);
  }
  if let Some(area) = req.area_sq_ft {
    if !(area > 0.0) {
      return Err(
        ApiError::BadRequest("area_sq_ft must be > 0".into()).into_response(),
      );
    }
    merged.area_sq_ft = area;
  }
  if let Some(notes) = req.notes {
    merged.notes = Some(notes);
  }

  // Mirror the change in SimWorld so the next sub-step reads the
  // new emitter / soil constants.
  {
    let mut world = state.world.0.lock().await;
    world.update_zone(merged.clone()).map_err(|e| {
      ApiError::Internal(format!("SimWorld update_zone failed: {e}"))
        .into_response()
    })?;
  }
  bundle.zones[idx] = merged.clone();
  info!(zone_id = %path.zone_id, "Updated zone");
  Ok(Json(ZoneDefinition::from(&merged)))
}

async fn delete_zone(
  State(state): State<AppState>,
  Path(path): Path<ZoneIdPath>,
) -> ApiResult<DeletedResponse> {
  let zone_id = ZoneId::new(path.zone_id.clone());
  let mut bundle = state.property.lock().await;
  let idx = bundle
    .zones
    .iter()
    .position(|z| z.id == zone_id)
    .ok_or_else(|| {
      ApiError::ZoneNotFound(format!("zone {}", path.zone_id)).into_response()
    })?;
  // SimWorld first; its return value confirms the zone was there
  // before we touched the property bundle.
  {
    let mut world = state.world.0.lock().await;
    world.remove_zone(&zone_id).map_err(api_err)?;
  }
  bundle.zones.remove(idx);
  info!(zone_id = %path.zone_id, "Deleted zone");
  Ok(Json(DeletedResponse {
    zone_id: path.zone_id,
  }))
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new()
    .api_route(
      "/api/zones/definitions",
      post_with(create_zone, |op: TransformOperation| {
        op.description("Add a zone to the property and the running simulator.")
      }),
    )
    .api_route(
      "/api/zones/definitions/{zone_id}",
      get_with(get_zone_definition, |op: TransformOperation| {
        op.description(
          "Return one zone's definition (not its operational status).",
        )
      })
      .patch_with(update_zone, |op: TransformOperation| {
        op.description(
          "Edit a zone's definition in place.  All fields optional.",
        )
      })
      .delete_with(delete_zone, |op: TransformOperation| {
        op.description("Remove a zone from the property and the simulator.")
      }),
    )
}
