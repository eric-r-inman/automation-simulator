//! Sensor readings routed through `Arc<dyn SensorSource>`.

use aide::axum::{routing::get_with, ApiRouter};
use aide::transform::TransformOperation;
use automation_simulator_lib::engine::SimInstant;
use automation_simulator_lib::sim::id::ZoneId;
use axum::extract::{Path, State};
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ZoneIdPath {
  pub zone_id: String,
}

use super::{api_err, ApiResult};
use crate::web_base::AppState;

#[derive(Debug, Serialize, JsonSchema)]
pub struct SensorsResponse {
  pub readings: Vec<ReadingDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadingDto {
  pub zone_id: String,
  pub kind: String,
  pub value: f64,
  pub taken_at_minutes: i64,
}

async fn latest_readings(
  State(state): State<AppState>,
) -> ApiResult<SensorsResponse> {
  // Walk the world's zones (stable fixture order), take the latest
  // reading per zone.  Zones with no reading yet are omitted rather
  // than returned as nulls so the UI can distinguish "no data" from
  // "zero moisture".
  let zone_ids: Vec<ZoneId> = {
    let guard = state.world.0.lock().await;
    guard.zones.iter().map(|z| z.id.clone()).collect()
  };
  let mut readings = Vec::with_capacity(zone_ids.len());
  for z in zone_ids {
    if let Some(r) = state.sensors.latest_reading(&z).await.map_err(api_err)? {
      readings.push(ReadingDto {
        zone_id: r.zone_id.as_str().to_string(),
        kind: format!("{:?}", r.kind).to_lowercase(),
        value: r.value,
        taken_at_minutes: r.taken_at.minutes(),
      });
    }
  }
  Ok(Json(SensorsResponse { readings }))
}

async fn zone_history(
  State(state): State<AppState>,
  Path(path): Path<ZoneIdPath>,
) -> ApiResult<SensorsResponse> {
  let zone = ZoneId::new(path.zone_id);
  let history = state
    .sensors
    .history(&zone, SimInstant::START)
    .await
    .map_err(api_err)?;
  Ok(Json(SensorsResponse {
    readings: history
      .into_iter()
      .map(|r| ReadingDto {
        zone_id: r.zone_id.as_str().to_string(),
        kind: format!("{:?}", r.kind).to_lowercase(),
        value: r.value,
        taken_at_minutes: r.taken_at.minutes(),
      })
      .collect(),
  }))
}

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new()
    .api_route(
      "/api/sensors",
      get_with(latest_readings, |op: TransformOperation| {
        op.description("Latest reading per zone.")
      }),
    )
    .api_route(
      "/api/sensors/{zone_id}/history",
      get_with(zone_history, |op: TransformOperation| {
        op.description("Full reading history for one zone.")
      }),
    )
}
