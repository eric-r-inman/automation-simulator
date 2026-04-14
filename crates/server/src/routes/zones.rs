//! Zone-level commands routed through `Arc<dyn Controller>`.
//!
//! `GET /api/zones` lists every zone's current status.  `POST
//! /api/zones/:id/run` opens the valve; `POST /api/zones/:id/stop`
//! closes it.  All three sit on the same trait the v0.2 real-
//! hardware driver will also impl.

use aide::axum::{
  routing::{get_with, post_with},
  ApiRouter,
};
use aide::transform::TransformOperation;
use automation_simulator_lib::engine::SimDuration;
use automation_simulator_lib::sim::id::ZoneId;
use axum::extract::{Path, State};
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::{api_err, ApiError, ApiResult};
use crate::web_base::AppState;
use axum::response::IntoResponse;

// ── DTOs ────────────────────────────────────────────────────────────────────

/// Path-params wrapper so aide can document the `:zone_id` segment.
/// axum extracts it by name from the URL pattern.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ZoneIdPath {
  pub zone_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ZonesResponse {
  pub zones: Vec<ZoneStatusDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ZoneStatusDto {
  pub zone_id: String,
  pub is_open: bool,
  pub open_until_minutes: Option<i64>,
  pub total_open_seconds: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunZoneRequest {
  pub duration_minutes: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RunZoneResponse {
  pub zone_id: String,
  pub opened_for_minutes: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StopZoneResponse {
  pub zone_id: String,
}

// ── Handlers ────────────────────────────────────────────────────────────────

async fn list_zones(State(state): State<AppState>) -> ApiResult<ZonesResponse> {
  let zones = state.controller.list_zones().await.map_err(api_err)?;
  Ok(Json(ZonesResponse {
    zones: zones
      .into_iter()
      .map(|z| ZoneStatusDto {
        zone_id: z.zone_id.as_str().to_string(),
        is_open: z.is_open,
        open_until_minutes: z.open_until.map(|i| i.minutes()),
        total_open_seconds: z.total_open_seconds,
      })
      .collect(),
  }))
}

async fn run_zone(
  State(state): State<AppState>,
  Path(path): Path<ZoneIdPath>,
  Json(req): Json<RunZoneRequest>,
) -> ApiResult<RunZoneResponse> {
  if req.duration_minutes <= 0 {
    return Err(
      ApiError::BadRequest("duration_minutes must be > 0".to_string())
        .into_response(),
    );
  }
  let zone = ZoneId::new(path.zone_id.clone());
  state
    .controller
    .open_zone(&zone, SimDuration::minutes(req.duration_minutes))
    .await
    .map_err(api_err)?;
  info!(zone = %path.zone_id, minutes = req.duration_minutes, "Opened zone");
  Ok(Json(RunZoneResponse {
    zone_id: path.zone_id,
    opened_for_minutes: req.duration_minutes,
  }))
}

async fn stop_zone(
  State(state): State<AppState>,
  Path(path): Path<ZoneIdPath>,
) -> ApiResult<StopZoneResponse> {
  let zone = ZoneId::new(path.zone_id.clone());
  state.controller.close_zone(&zone).await.map_err(api_err)?;
  info!(zone = %path.zone_id, "Closed zone");
  Ok(Json(StopZoneResponse {
    zone_id: path.zone_id,
  }))
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new()
    .api_route(
      "/api/zones",
      get_with(list_zones, |op: TransformOperation| {
        op.description("Snapshot every zone's current valve state.")
      }),
    )
    .api_route(
      "/api/zones/{zone_id}/run",
      post_with(run_zone, |op: TransformOperation| {
        op.description("Open a zone's valve for a duration in minutes.")
      }),
    )
    .api_route(
      "/api/zones/{zone_id}/stop",
      post_with(stop_zone, |op: TransformOperation| {
        op.description("Close a zone's valve immediately.")
      }),
    )
}
