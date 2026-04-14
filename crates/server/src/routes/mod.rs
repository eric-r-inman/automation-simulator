//! HTTP route modules for the simulator.
//!
//! Each file exposes a `router()` function that returns an
//! `ApiRouter<AppState>`.  `create_app` in `main.rs` mounts them
//! side-by-side so a new route module is a one-line wiring change.
//! The trait-backed routes (`zones.rs`, `sensors.rs`, `weather.rs`)
//! speak to `Arc<dyn Controller>` / `Arc<dyn SensorSource>` on
//! `AppState`, so v0.2 real-hardware drivers land without touching
//! this file.

pub mod catalog;
pub mod planner;
pub mod sensors;
pub mod sim;
pub mod weather;
pub mod zones;
pub mod zones_crud;

use automation_simulator_lib::engine::SimWorldError;
use automation_simulator_lib::hw::{ControllerError, SensorError};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Wraps errors from the trait layer into HTTP responses.  Each
/// variant maps to a specific status code; the response body is a
/// small JSON document with `error` and `kind` fields so clients
/// can branch on the kind without string-matching.
#[derive(Debug)]
pub enum ApiError {
  ZoneNotFound(String),
  BadRequest(String),
  Upstream(String),
  Internal(String),
}

impl ApiError {
  fn status(&self) -> StatusCode {
    match self {
      Self::ZoneNotFound(_) => StatusCode::NOT_FOUND,
      Self::BadRequest(_) => StatusCode::BAD_REQUEST,
      Self::Upstream(_) => StatusCode::BAD_GATEWAY,
      Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  fn kind(&self) -> &'static str {
    match self {
      Self::ZoneNotFound(_) => "zone-not-found",
      Self::BadRequest(_) => "bad-request",
      Self::Upstream(_) => "upstream-unreachable",
      Self::Internal(_) => "internal-error",
    }
  }

  fn message(&self) -> &str {
    match self {
      Self::ZoneNotFound(m)
      | Self::BadRequest(m)
      | Self::Upstream(m)
      | Self::Internal(m) => m,
    }
  }
}

impl From<ControllerError> for ApiError {
  fn from(e: ControllerError) -> Self {
    match e {
      ControllerError::ZoneNotFound(id) => {
        Self::ZoneNotFound(format!("zone {id}"))
      }
      ControllerError::ZoneOpen { .. } | ControllerError::ZoneClose { .. } => {
        Self::BadRequest(e.to_string())
      }
      ControllerError::Unreachable(_) | ControllerError::Timeout { .. } => {
        Self::Upstream(e.to_string())
      }
    }
  }
}

impl From<SensorError> for ApiError {
  fn from(e: SensorError) -> Self {
    match e {
      SensorError::ZoneNotFound(id) => Self::ZoneNotFound(format!("zone {id}")),
      SensorError::Unreachable(_) | SensorError::Timeout { .. } => {
        Self::Upstream(e.to_string())
      }
    }
  }
}

impl From<SimWorldError> for ApiError {
  fn from(e: SimWorldError) -> Self {
    match e {
      SimWorldError::UnknownZone(id) => {
        Self::ZoneNotFound(format!("zone {id}"))
      }
      SimWorldError::DuplicateZone(id) => {
        Self::BadRequest(format!("zone {id} already exists"))
      }
      SimWorldError::UnknownSoilType(_, _)
      | SimWorldError::UnknownEmitterSpec(_, _)
      | SimWorldError::UnknownClimateZone(_) => Self::BadRequest(e.to_string()),
    }
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    let body = Json(json!({
      "error": self.message(),
      "kind": self.kind(),
    }));
    (self.status(), body).into_response()
  }
}

/// Type alias used by every handler in this module's submodules:
/// the success branch carries a typed JSON body that aide can
/// document; the error branch is an axum `Response` (which aide
/// already knows how to handle as `OperationOutput`).  Convert
/// `ApiError` values at the `?` boundary with `.map_err(...)?` or
/// `.into_response()`.
pub type ApiResult<T> = Result<axum::Json<T>, axum::response::Response>;

/// Convenience: convert any `ControllerError` / `SensorError` into
/// a response in one call so handlers can write
/// `.await.map_err(api_err)?`.
pub fn api_err<E: Into<ApiError>>(e: E) -> Response {
  e.into().into_response()
}
