//! Multi-property registry routes.
//!
//! The v0.3 Properties page lists every property the server has
//! seen this process and lets the user switch the running
//! simulator between them.  The registry lives on `AppState` as a
//! `BTreeMap<String, PropertyBundle>`; the currently-active
//! property is mirrored into `AppState.property` + `AppState.world`
//! so all existing routes keep working unchanged.
//!
//! Persistence is deliberately in-memory in v0.3.  A follow-up will
//! move the registry to SQLite so a restart does not wipe the list.

use aide::axum::{
  routing::{delete_with, get_with, post_with},
  ApiRouter,
};
use aide::transform::TransformOperation;
use automation_simulator_lib::engine::SimWorld;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use super::{ApiError, ApiResult};
use crate::web_base::AppState;

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PropertyIdPath {
  pub property_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PropertyListEntry {
  pub id: String,
  pub name: String,
  pub climate_zone: String,
  pub zones: usize,
  pub yards: usize,
  pub active: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PropertiesResponse {
  pub properties: Vec<PropertyListEntry>,
  pub active_property_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ActivatedResponse {
  pub property_id: String,
  pub property_name: String,
  pub zones: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeletedPropertyResponse {
  pub property_id: String,
}

// ── Handlers ────────────────────────────────────────────────────────────────

async fn list_properties(
  State(state): State<AppState>,
) -> ApiResult<PropertiesResponse> {
  let active_id = {
    let bundle = state.property.lock().await;
    bundle.property.id.as_str().to_string()
  };
  let registry = state.properties.lock().await;
  let properties: Vec<PropertyListEntry> = registry
    .values()
    .map(|b| PropertyListEntry {
      id: b.property.id.as_str().to_string(),
      name: b.property.name.clone(),
      climate_zone: b.property.climate_zone.clone(),
      zones: b.zones.len(),
      yards: b.property.yards.len(),
      active: b.property.id.as_str() == active_id,
    })
    .collect();
  Ok(Json(PropertiesResponse {
    properties,
    active_property_id: active_id,
  }))
}

async fn activate_property(
  State(state): State<AppState>,
  Path(path): Path<PropertyIdPath>,
) -> ApiResult<ActivatedResponse> {
  // Copy the requested bundle out of the registry so we can rebuild
  // the sim world without holding the registry lock across an
  // `.await`.
  let bundle = {
    let registry = state.properties.lock().await;
    registry.get(&path.property_id).cloned().ok_or_else(|| {
      ApiError::ZoneNotFound(format!("property {}", path.property_id))
        .into_response()
    })?
  };

  let new_world = SimWorld::new(
    chrono::NaiveDate::from_ymd_opt(2026, 7, 1).expect("valid date"),
    &bundle.property.climate_zone,
    bundle.zones.clone(),
    Arc::clone(&state.catalog),
    1,
    0.30,
    Vec::new(),
  )
  .map_err(|e| {
    ApiError::Internal(format!(
      "failed to build sim world for property {}: {e}",
      path.property_id
    ))
    .into_response()
  })?;

  {
    let mut active = state.property.lock().await;
    *active = bundle.clone();
  }
  {
    let mut world = state.world.0.lock().await;
    *world = new_world;
  }

  info!(
    property_id = %path.property_id,
    "Activated property — running simulator swapped"
  );

  Ok(Json(ActivatedResponse {
    property_id: bundle.property.id.as_str().to_string(),
    property_name: bundle.property.name,
    zones: bundle.zones.len(),
  }))
}

async fn delete_property(
  State(state): State<AppState>,
  Path(path): Path<PropertyIdPath>,
) -> ApiResult<DeletedPropertyResponse> {
  let active_id = {
    let bundle = state.property.lock().await;
    bundle.property.id.as_str().to_string()
  };
  if active_id == path.property_id {
    return Err(
      ApiError::BadRequest(format!(
        "cannot delete property {} while it is the active simulator; \
         activate a different property first",
        path.property_id
      ))
      .into_response(),
    );
  }
  let mut registry = state.properties.lock().await;
  if registry.remove(&path.property_id).is_none() {
    return Err(
      ApiError::ZoneNotFound(format!("property {}", path.property_id))
        .into_response(),
    );
  }
  info!(property_id = %path.property_id, "Deleted property from registry");
  Ok(Json(DeletedPropertyResponse {
    property_id: path.property_id,
  }))
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new()
    .api_route(
      "/api/properties",
      get_with(list_properties, |op: TransformOperation| {
        op.description(
          "List every property the server has in its in-memory registry, \
           plus which one the simulator is currently running.",
        )
      }),
    )
    .api_route(
      "/api/properties/{property_id}/activate",
      post_with(activate_property, |op: TransformOperation| {
        op.description(
          "Replace the running simulator's world with the named property. \
           All subsequent /api/sim/* calls serve the newly-activated one.",
        )
      }),
    )
    .api_route(
      "/api/properties/{property_id}",
      delete_with(delete_property, |op: TransformOperation| {
        op.description(
          "Remove a property from the registry.  Refuses to remove the \
           currently-active property; activate a different one first.",
        )
      }),
    )
}
