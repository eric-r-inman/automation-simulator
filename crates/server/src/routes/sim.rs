//! Simulator-control routes.
//!
//! These endpoints expose the simulator as a whole — property
//! snapshot, current state, time advance, reset.  Real hardware
//! has no equivalent for `/api/sim/step`, so the simulator-only
//! routes live in their own module; in v0.2 these stay wired but
//! become 400s when `hardware_mode = real`.

use aide::axum::{
  routing::{get_with, post_with},
  ApiRouter,
};
use aide::transform::TransformOperation;
use automation_simulator_lib::engine::{SimDuration, SimWorld};
use axum::extract::State;
use axum::Json;
use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use super::{ApiError, ApiResult};
use crate::web_base::AppState;

// ── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, JsonSchema)]
pub struct PropertyResponse {
  pub id: String,
  pub name: String,
  pub climate_zone: String,
  pub lot_area_sq_ft: f64,
  pub yards: Vec<YardSummary>,
  pub spigots: Vec<SpigotSummary>,
  pub zones: Vec<ZoneSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct YardSummary {
  pub id: String,
  pub name: String,
  pub area_sq_ft: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SpigotSummary {
  pub id: String,
  pub mains_pressure_psi: f64,
  pub notes: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ZoneSummary {
  pub id: String,
  pub yard_id: String,
  pub manifold_id: String,
  pub plant_kind: String,
  pub area_sq_ft: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SimStateResponse {
  pub simulated_minutes_elapsed: i64,
  pub simulated_datetime: String,
  pub zones: Vec<ZoneState>,
  pub weather: WeatherState,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ZoneState {
  pub zone_id: String,
  pub soil_vwc: f64,
  pub valve_is_open: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct WeatherState {
  pub temperature_c: f64,
  pub humidity_pct: f64,
  pub wind_m_per_s: f64,
  pub solar_w_per_m2: f64,
  pub precipitation_mm_per_hour: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepRequest {
  /// Simulated minutes to advance the clock by.  Must be > 0.
  pub minutes: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StepResponse {
  pub minutes_advanced: i64,
  pub simulated_minutes_elapsed: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResetResponse {
  pub reset_at: String,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn get_property(State(state): State<AppState>) -> Json<PropertyResponse> {
  let p = &state.property.property;
  Json(PropertyResponse {
    id: p.id.as_str().to_string(),
    name: p.name.clone(),
    climate_zone: p.climate_zone.clone(),
    lot_area_sq_ft: p.lot_area_sq_ft,
    yards: p
      .yards
      .iter()
      .map(|y| YardSummary {
        id: y.id.as_str().to_string(),
        name: y.name.clone(),
        area_sq_ft: y.area_sq_ft,
      })
      .collect(),
    spigots: p
      .spigots
      .iter()
      .map(|s| SpigotSummary {
        id: s.id.as_str().to_string(),
        mains_pressure_psi: s.mains_pressure_psi,
        notes: s.notes.clone(),
      })
      .collect(),
    zones: state
      .property
      .zones
      .iter()
      .map(|z| ZoneSummary {
        id: z.id.as_str().to_string(),
        yard_id: z.yard_id.as_str().to_string(),
        manifold_id: z.manifold_id.as_str().to_string(),
        plant_kind: format!("{:?}", z.plant_kind).to_lowercase(),
        area_sq_ft: z.area_sq_ft,
      })
      .collect(),
  })
}

async fn get_state(State(state): State<AppState>) -> Json<SimStateResponse> {
  let mut guard = state.world.0.lock().await;
  let now = guard.clock.now();
  let dt = guard.clock.to_datetime(now);
  let clock = guard.clock.clone();
  let weather_sample = guard.weather.sample_at(&clock, now);

  let zones = guard
    .zones
    .iter()
    .map(|z| {
      let valve = guard.valves.get(&z.id).copied().unwrap_or_default();
      let vwc = guard.soil.get(&z.id).copied().map(|s| s.vwc).unwrap_or(0.0);
      ZoneState {
        zone_id: z.id.as_str().to_string(),
        soil_vwc: vwc,
        valve_is_open: valve.is_open(now),
      }
    })
    .collect();

  Json(SimStateResponse {
    simulated_minutes_elapsed: now.minutes(),
    simulated_datetime: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
    zones,
    weather: WeatherState {
      temperature_c: weather_sample.temperature_c,
      humidity_pct: weather_sample.humidity_pct,
      wind_m_per_s: weather_sample.wind_m_per_s,
      solar_w_per_m2: weather_sample.solar_w_per_m2,
      precipitation_mm_per_hour: weather_sample.precipitation_mm_per_hour,
    },
  })
}

async fn post_step(
  State(state): State<AppState>,
  Json(req): Json<StepRequest>,
) -> ApiResult<StepResponse> {
  use axum::response::IntoResponse;
  if req.minutes <= 0 {
    return Err(
      ApiError::BadRequest("minutes must be > 0".to_string()).into_response(),
    );
  }
  let mut guard = state.world.0.lock().await;
  guard.advance(SimDuration::minutes(req.minutes));
  let now_minutes = guard.clock.now().minutes();
  info!(minutes = req.minutes, "Advanced simulation");
  Ok(Json(StepResponse {
    minutes_advanced: req.minutes,
    simulated_minutes_elapsed: now_minutes,
  }))
}

async fn post_reset(State(state): State<AppState>) -> ApiResult<ResetResponse> {
  use axum::response::IntoResponse;
  // Rebuild SimWorld with the same inputs and swap it into the
  // mutex.  Bounces all per-zone soil and valve state + clears
  // history.  Uses the same seed so determinism holds.
  let zones = state.property.zones.clone();
  let climate_zone = state.property.property.climate_zone.clone();
  let new_world = SimWorld::new(
    NaiveDate::from_ymd_opt(2026, 7, 1).expect("valid date"),
    &climate_zone,
    zones,
    Arc::clone(&state.catalog),
    1,
    0.30,
    Vec::new(),
  )
  .map_err(|e| ApiError::Internal(e.to_string()).into_response())?;

  let mut guard = state.world.0.lock().await;
  *guard = new_world;
  let dt = guard.clock.to_datetime(guard.clock.now());
  info!("Simulation reset");
  Ok(Json(ResetResponse {
    reset_at: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
  }))
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new()
    .api_route(
      "/api/sim/property",
      get_with(get_property, |op: TransformOperation| {
        op.description("Return the loaded property's geometry and zones.")
      }),
    )
    .api_route(
      "/api/sim/state",
      get_with(get_state, |op: TransformOperation| {
        op.description(
          "Return the simulator's current clock, per-zone moisture + \
           valve state, and current weather.",
        )
      }),
    )
    .api_route(
      "/api/sim/step",
      post_with(post_step, |op: TransformOperation| {
        op.description("Advance the simulation clock by N minutes.")
      }),
    )
    .api_route(
      "/api/sim/reset",
      post_with(post_reset, |op: TransformOperation| {
        op.description("Reset the simulator to the initial state.")
      }),
    )
}
