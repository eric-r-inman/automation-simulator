//! Current weather routed through `Arc<dyn SensorSource>::weather_now`.

use aide::axum::{routing::get_with, ApiRouter};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::Json;
use schemars::JsonSchema;
use serde::Serialize;

use super::{api_err, ApiResult};
use crate::web_base::AppState;

#[derive(Debug, Serialize, JsonSchema)]
pub struct WeatherResponse {
  pub temperature_c: f64,
  pub humidity_pct: f64,
  pub wind_m_per_s: f64,
  pub solar_w_per_m2: f64,
  pub precipitation_mm_per_hour: f64,
}

async fn current_weather(
  State(state): State<AppState>,
) -> ApiResult<WeatherResponse> {
  let w = state.sensors.weather_now().await.map_err(api_err)?;
  Ok(Json(WeatherResponse {
    temperature_c: w.temperature_c,
    humidity_pct: w.humidity_pct,
    wind_m_per_s: w.wind_m_per_s,
    solar_w_per_m2: w.solar_w_per_m2,
    precipitation_mm_per_hour: w.precipitation_mm_per_hour,
  }))
}

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new().api_route(
    "/api/weather",
    get_with(current_weather, |op: TransformOperation| {
      op.description("Current weather at the simulated property.")
    }),
  )
}
