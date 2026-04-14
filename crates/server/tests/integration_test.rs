use automation_simulator_lib::{
  catalog::Catalog,
  engine::SimWorld,
  hw::{
    Controller, SensorSource, SharedWorld, SimulatedController,
    SimulatedSensorSource,
  },
  seed::load_property,
};
use automation_simulator_server::web_base::{base_router, AppState};
use axum::{
  body::Body,
  http::{Request, StatusCode},
  Router,
};
use chrono::NaiveDate;
use openidconnect::{
  core::{
    CoreClient, CoreJwsSigningAlgorithm, CoreProviderMetadata,
    CoreResponseType, CoreSubjectIdentifierType,
  },
  AuthUrl, ClientId, EmptyAdditionalProviderMetadata, IssuerUrl,
  JsonWebKeySetUrl, ResponseTypes,
};
use prometheus::{IntCounter, Registry};
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;
use tower_sessions::{cookie::SameSite, MemoryStore, SessionManagerLayer};

// ── shared fixture loader ────────────────────────────────────────────────────

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
}

fn property_fixture_path() -> PathBuf {
  workspace_root()
    .join("data")
    .join("properties")
    .join("example-property.toml")
}

fn catalog_dir() -> PathBuf {
  workspace_root().join("data").join("catalog")
}

/// Build the simulator pieces that every AppState test needs.  Uses
/// the real example-property fixture + catalog so the routes have a
/// realistic graph to operate on.
fn build_sim_pieces() -> (
  SharedWorld,
  Arc<dyn Controller>,
  Arc<dyn SensorSource>,
  Arc<Catalog>,
  Arc<tokio::sync::Mutex<automation_simulator_lib::seed::PropertyBundle>>,
) {
  let catalog = Arc::new(Catalog::load(catalog_dir()).expect("catalog"));
  let bundle =
    load_property(property_fixture_path(), &catalog).expect("bundle");
  let zones = bundle.zones.clone();
  let world = SimWorld::new(
    NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
    &bundle.property.climate_zone,
    zones,
    Arc::clone(&catalog),
    1,
    0.30,
    Vec::new(),
  )
  .expect("sim world");
  let shared = SharedWorld::new(world);
  let controller: Arc<dyn Controller> =
    Arc::new(SimulatedController::new(shared.clone()));
  let sensors: Arc<dyn SensorSource> =
    Arc::new(SimulatedSensorSource::new(shared.clone()));
  (
    shared,
    controller,
    sensors,
    catalog,
    Arc::new(tokio::sync::Mutex::new(bundle)),
  )
}

// ── state helpers ────────────────────────────────────────────────────────────

fn stub_state_no_auth(frontend_path: PathBuf) -> AppState {
  let registry = Registry::new();
  let request_counter =
    IntCounter::new("http_requests_total", "Total HTTP requests")
      .expect("counter creation");
  registry
    .register(Box::new(request_counter.clone()))
    .expect("counter registration");

  let (world, controller, sensors, catalog, property) = build_sim_pieces();

  AppState {
    registry: Arc::new(registry),
    request_counter,
    frontend_path,
    oidc_client: None,
    world,
    controller,
    sensors,
    catalog,
    property,
  }
}

fn stub_state_with_auth(frontend_path: PathBuf) -> AppState {
  let registry = Registry::new();
  let request_counter =
    IntCounter::new("http_requests_total", "Total HTTP requests")
      .expect("counter creation");
  registry
    .register(Box::new(request_counter.clone()))
    .expect("counter registration");

  let issuer = IssuerUrl::new("https://stub.invalid".to_string()).unwrap();
  let metadata = CoreProviderMetadata::new(
    issuer,
    AuthUrl::new("https://stub.invalid/authorize".to_string()).unwrap(),
    JsonWebKeySetUrl::new("https://stub.invalid/jwks".to_string()).unwrap(),
    vec![ResponseTypes::new(vec![CoreResponseType::Code])],
    vec![CoreSubjectIdentifierType::Public],
    vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256],
    EmptyAdditionalProviderMetadata {},
  );
  let oidc_client = CoreClient::from_provider_metadata(
    metadata,
    ClientId::new("stub-client".to_string()),
    None,
  );

  let (world, controller, sensors, catalog, property) = build_sim_pieces();

  AppState {
    registry: Arc::new(registry),
    request_counter,
    frontend_path,
    oidc_client: Some(Arc::new(oidc_client)),
    world,
    controller,
    sensors,
    catalog,
    property,
  }
}

fn state_without_frontend() -> AppState {
  stub_state_no_auth(PathBuf::from("/nonexistent"))
}

/// Wraps `base_router` with auth routes and a session layer, mirroring
/// the production `create_app` structure.
fn app_with_session(state: AppState) -> Router {
  use automation_simulator_server::auth;
  use axum::routing::get;

  let session_store = MemoryStore::default();
  let session_layer = SessionManagerLayer::new(session_store)
    .with_secure(false)
    .with_same_site(SameSite::Lax);

  let auth_router = Router::new()
    .route("/auth/login", get(auth::login_handler))
    .route("/auth/callback", get(auth::callback_handler))
    .route("/auth/logout", get(auth::logout_handler))
    .with_state(state.clone());

  base_router(state).merge(auth_router).layer(session_layer)
}

/// Mounts the simulator route modules on top of `base_router`,
/// matching `main::create_app`'s composition.  Tests use this to
/// hit `/api/sim/*`, `/api/zones`, `/api/sensors`, `/api/weather`.
fn app_with_sim_routes(state: AppState) -> Router {
  use automation_simulator_server::routes;
  let sim_routes: Router = Router::<()>::from(
    routes::sim::router()
      .merge(routes::zones::router())
      .merge(routes::zones_crud::router())
      .merge(routes::sensors::router())
      .merge(routes::weather::router())
      .merge(routes::catalog::router())
      .merge(routes::planner::router())
      .with_state(state.clone()),
  );
  base_router(state).merge(sim_routes)
}

// ── existing route tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_healthz_endpoint() {
  let app = base_router(state_without_frontend());

  let response = app
    .oneshot(
      Request::builder()
        .uri("/healthz")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);

  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .unwrap();
  let body_str = String::from_utf8(body.to_vec()).unwrap();

  assert!(body_str.contains("healthy"));
}

#[tokio::test]
async fn test_metrics_endpoint() {
  let app = base_router(state_without_frontend());

  let response = app
    .oneshot(
      Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);

  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .unwrap();
  let body_str = String::from_utf8(body.to_vec()).unwrap();

  assert!(
    body_str.contains("http_requests_total"),
    "Metrics should contain http_requests_total counter"
  );
}

#[tokio::test]
async fn test_openapi_json_endpoint() {
  let app = base_router(state_without_frontend());

  let response = app
    .oneshot(
      Request::builder()
        .uri("/api-docs/openapi.json")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);

  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .unwrap();
  let body_str = String::from_utf8(body.to_vec()).unwrap();

  assert!(body_str.contains("openapi"), "Response should be an OpenAPI spec");
  assert!(body_str.contains("/healthz"), "Spec should document /healthz");
  assert!(body_str.contains("/metrics"), "Spec should document /metrics");
}

#[tokio::test]
async fn test_scalar_ui_endpoint() {
  let app = base_router(state_without_frontend());

  let response = app
    .oneshot(
      Request::builder()
        .uri("/scalar")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);

  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .unwrap();

  assert!(
    body.starts_with(b"<!doctype html>")
      || body.starts_with(b"<!DOCTYPE html>"),
    "Scalar endpoint should return HTML"
  );
}

#[tokio::test]
async fn test_spa_fallback_serves_index_html() {
  let frontend_dir = tempfile::tempdir().unwrap();
  std::fs::write(
    frontend_dir.path().join("index.html"),
    b"<!doctype html><title>automation-simulator</title>",
  )
  .unwrap();

  let app = base_router(stub_state_no_auth(frontend_dir.path().to_path_buf()));

  for path in ["/some-page", "/nested/route", "/unknown"] {
    let response = app
      .clone()
      .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
      .await
      .unwrap();
    assert_eq!(
      response.status(),
      StatusCode::OK,
      "expected 200 for SPA path {path}"
    );
  }
}

// ── /me endpoint tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_me_no_oidc() {
  let state = stub_state_no_auth(PathBuf::from("/nonexistent"));
  let app = app_with_session(state);

  let response = app
    .oneshot(Request::builder().uri("/me").body(Body::empty()).unwrap())
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);

  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .unwrap();
  let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

  assert_eq!(json["name"], "admin");
  assert_eq!(json["auth_enabled"], false);
}

#[tokio::test]
async fn test_me_with_oidc_no_session() {
  let state = stub_state_with_auth(PathBuf::from("/nonexistent"));
  let app = app_with_session(state);

  let response = app
    .oneshot(Request::builder().uri("/me").body(Body::empty()).unwrap())
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);

  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .unwrap();
  let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

  assert_eq!(json["name"], "anonymous");
  assert_eq!(json["auth_enabled"], true);
}

// ── auth route guard tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_auth_routes_return_404_without_oidc() {
  let state = stub_state_no_auth(PathBuf::from("/nonexistent"));
  let app = app_with_session(state);

  for path in ["/auth/login", "/auth/logout"] {
    let response = app
      .clone()
      .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
      .await
      .unwrap();
    assert_eq!(
      response.status(),
      StatusCode::NOT_FOUND,
      "expected 404 for {path} without OIDC"
    );
  }

  // callback needs query params; without them Axum rejects before our guard,
  // but we can confirm it still doesn't 500 or 200.
  let response = app
    .oneshot(
      Request::builder()
        .uri("/auth/callback?code=x&state=y")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_auth_login_redirects_with_oidc() {
  let state = stub_state_with_auth(PathBuf::from("/nonexistent"));
  let app = app_with_session(state);

  let response = app
    .oneshot(
      Request::builder()
        .uri("/auth/login")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  // The stub provider's authorize URL should trigger a redirect.
  assert_eq!(response.status(), StatusCode::SEE_OTHER);
  let location = response
    .headers()
    .get("location")
    .expect("redirect should have Location header")
    .to_str()
    .unwrap();
  assert!(
    location.contains("stub.invalid"),
    "redirect should point at the stub OIDC provider"
  );
}

// ── config tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_config_no_oidc() {
  use automation_simulator_server::config::{CliRaw, Config};

  let cli = CliRaw {
    log_level: None,
    log_format: None,
    config: None,
    listen: None,
    frontend_path: None,
    base_url: Some("https://example.com".to_string()),
    oidc_issuer: None,
    oidc_client_id: None,
    oidc_client_secret_file: None,
    property_path: Some(property_fixture_path()),
    catalog_path: Some(catalog_dir()),
  };

  let config = Config::from_cli_and_file(cli).expect("config");
  assert!(config.oidc.is_none());
  assert_eq!(config.property_path, property_fixture_path());
  assert_eq!(config.catalog_path, catalog_dir());
}

#[tokio::test]
async fn test_config_full_oidc() {
  use automation_simulator_server::config::{CliRaw, Config};

  let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests/fixtures/oidc-client-secret");

  let cli = CliRaw {
    log_level: None,
    log_format: None,
    config: None,
    listen: None,
    frontend_path: None,
    base_url: Some("https://example.com".to_string()),
    oidc_issuer: Some("https://sso.example.com".to_string()),
    oidc_client_id: Some("my-client".to_string()),
    oidc_client_secret_file: Some(fixture),
    property_path: Some(property_fixture_path()),
    catalog_path: Some(catalog_dir()),
  };

  let config = Config::from_cli_and_file(cli).unwrap();
  let oidc = config.oidc.expect("OIDC config should be Some");
  assert_eq!(oidc.issuer, "https://sso.example.com");
  assert_eq!(oidc.client_id, "my-client");
  assert_eq!(oidc.client_secret, "test-secret-not-for-production");
}

#[tokio::test]
async fn test_config_partial_oidc_errors() {
  use automation_simulator_server::config::{CliRaw, Config};

  let cli = CliRaw {
    log_level: None,
    log_format: None,
    config: None,
    listen: None,
    frontend_path: None,
    base_url: Some("https://example.com".to_string()),
    oidc_issuer: Some("https://sso.example.com".to_string()),
    oidc_client_id: None,
    oidc_client_secret_file: None,
    property_path: Some(property_fixture_path()),
    catalog_path: Some(catalog_dir()),
  };

  let err = Config::from_cli_and_file(cli).unwrap_err();
  let msg = err.to_string();
  assert!(
    msg.contains("partial OIDC") && msg.contains("missing"),
    "error should describe partial OIDC config, got: {msg}"
  );
}

// ── simulator route tests ────────────────────────────────────────────────────

async fn json_get(app: &Router, uri: &str) -> serde_json::Value {
  let response = app
    .clone()
    .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
    .await
    .expect("request");
  assert_eq!(response.status(), StatusCode::OK, "GET {uri} should be 200");
  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .expect("body");
  serde_json::from_slice(&body).expect("json")
}

async fn json_post(
  app: &Router,
  uri: &str,
  body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
  let response = app
    .clone()
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap(),
    )
    .await
    .expect("request");
  let status = response.status();
  let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .expect("body");
  let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
  (status, value)
}

#[tokio::test]
async fn test_get_property_returns_loaded_fixture() {
  let app = app_with_sim_routes(state_without_frontend());
  let body = json_get(&app, "/api/sim/property").await;
  assert_eq!(body["id"], "example-property");
  assert_eq!(body["climate_zone"], "portland-or");
  assert_eq!(body["yards"].as_array().unwrap().len(), 2);
  assert_eq!(body["spigots"].as_array().unwrap().len(), 2);
  assert_eq!(body["manifolds"].as_array().unwrap().len(), 2);
  assert_eq!(body["zones"].as_array().unwrap().len(), 6);
  // First manifold should carry its catalog model id and capacity.
  let first = &body["manifolds"][0];
  assert!(first["model_id"].is_string());
  assert!(first["zone_capacity"].is_number());
}

#[tokio::test]
async fn test_get_catalog_returns_emitters_soils_species() {
  let app = app_with_sim_routes(state_without_frontend());
  let body = json_get(&app, "/api/catalog").await;
  let emitters = body["emitters"].as_array().unwrap();
  let soils = body["soil_types"].as_array().unwrap();
  let species = body["species"].as_array().unwrap();
  assert!(!emitters.is_empty(), "expected at least one emitter");
  assert!(!soils.is_empty(), "expected at least one soil type");
  assert!(!species.is_empty(), "expected at least one species");
  // Every entry must have an id and a human-readable name.
  for e in emitters {
    assert!(e["id"].is_string());
    assert!(e["name"].is_string());
  }
}

#[tokio::test]
async fn test_get_state_returns_zones_and_weather() {
  let app = app_with_sim_routes(state_without_frontend());
  let body = json_get(&app, "/api/sim/state").await;
  assert_eq!(body["simulated_minutes_elapsed"], 0);
  assert_eq!(body["zones"].as_array().unwrap().len(), 6);
  let first = &body["zones"][0];
  assert!(first["soil_vwc"].as_f64().unwrap() > 0.0);
  assert_eq!(first["valve_is_open"], false);
  // Weather block should carry a plausible July temperature.
  let temp = body["weather"]["temperature_c"].as_f64().unwrap();
  assert!((10.0..40.0).contains(&temp));
}

#[tokio::test]
async fn test_step_advances_clock() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) =
    json_post(&app, "/api/sim/step", serde_json::json!({ "minutes": 60 }))
      .await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(body["minutes_advanced"], 60);
  assert_eq!(body["simulated_minutes_elapsed"], 60);
}

#[tokio::test]
async fn test_step_rejects_non_positive_minutes() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) =
    json_post(&app, "/api/sim/step", serde_json::json!({ "minutes": 0 })).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert_eq!(body["kind"], "bad-request");
}

#[tokio::test]
async fn test_reset_returns_to_initial_state() {
  let app = app_with_sim_routes(state_without_frontend());
  // Advance, then reset, then check elapsed is back to zero.
  let _ =
    json_post(&app, "/api/sim/step", serde_json::json!({ "minutes": 120 }))
      .await;
  let (status, _) =
    json_post(&app, "/api/sim/reset", serde_json::json!({})).await;
  assert_eq!(status, StatusCode::OK);
  let body = json_get(&app, "/api/sim/state").await;
  assert_eq!(body["simulated_minutes_elapsed"], 0);
}

#[tokio::test]
async fn test_list_zones_returns_six_zones() {
  let app = app_with_sim_routes(state_without_frontend());
  let body = json_get(&app, "/api/zones").await;
  let zones = body["zones"].as_array().unwrap();
  assert_eq!(zones.len(), 6);
  for z in zones {
    assert!(z["zone_id"].is_string());
    assert_eq!(z["is_open"], false);
    assert_eq!(z["total_open_seconds"], 0);
  }
}

#[tokio::test]
async fn test_run_zone_then_state_shows_open() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_post(
    &app,
    "/api/zones/zone-a1-veggies/run",
    serde_json::json!({ "duration_minutes": 15 }),
  )
  .await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(body["opened_for_minutes"], 15);
  assert_eq!(body["zone_id"], "zone-a1-veggies");

  let zones = json_get(&app, "/api/zones").await;
  let target = zones["zones"]
    .as_array()
    .unwrap()
    .iter()
    .find(|z| z["zone_id"] == "zone-a1-veggies")
    .expect("zone present");
  assert_eq!(target["is_open"], true);
}

#[tokio::test]
async fn test_run_unknown_zone_returns_404() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_post(
    &app,
    "/api/zones/no-such-zone/run",
    serde_json::json!({ "duration_minutes": 10 }),
  )
  .await;
  assert_eq!(status, StatusCode::NOT_FOUND);
  assert_eq!(body["kind"], "zone-not-found");
}

#[tokio::test]
async fn test_stop_zone_clears_open_state() {
  let app = app_with_sim_routes(state_without_frontend());
  let _ = json_post(
    &app,
    "/api/zones/zone-a1-veggies/run",
    serde_json::json!({ "duration_minutes": 15 }),
  )
  .await;
  let (status, _) =
    json_post(&app, "/api/zones/zone-a1-veggies/stop", serde_json::json!({}))
      .await;
  assert_eq!(status, StatusCode::OK);
  let zones = json_get(&app, "/api/zones").await;
  let target = zones["zones"]
    .as_array()
    .unwrap()
    .iter()
    .find(|z| z["zone_id"] == "zone-a1-veggies")
    .unwrap();
  assert_eq!(target["is_open"], false);
}

#[tokio::test]
async fn test_sensors_empty_until_advance() {
  let app = app_with_sim_routes(state_without_frontend());
  // No sub-steps yet → no recorded readings.
  let body = json_get(&app, "/api/sensors").await;
  assert!(body["readings"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_sensors_populated_after_step() {
  let app = app_with_sim_routes(state_without_frontend());
  // Advance past the recording cadence (60 sim-min default) so each
  // zone produces at least one sample.
  let _ =
    json_post(&app, "/api/sim/step", serde_json::json!({ "minutes": 120 }))
      .await;
  let body = json_get(&app, "/api/sensors").await;
  let readings = body["readings"].as_array().unwrap();
  assert_eq!(readings.len(), 6);
  for r in readings {
    assert_eq!(r["kind"], "soilvwc");
    assert!(r["value"].as_f64().unwrap() > 0.0);
  }
}

#[tokio::test]
async fn test_zone_history_returns_per_step_samples() {
  let app = app_with_sim_routes(state_without_frontend());
  let _ =
    json_post(&app, "/api/sim/step", serde_json::json!({ "minutes": 240 }))
      .await;
  let body = json_get(&app, "/api/sensors/zone-a1-veggies/history").await;
  let readings = body["readings"].as_array().unwrap();
  // Default record cadence is 60 sim-min, so 240 minutes ≈ 4 samples.
  assert!(readings.len() >= 3, "expected ≥3 samples, got {}", readings.len());
}

#[tokio::test]
async fn test_weather_endpoint_returns_sample() {
  let app = app_with_sim_routes(state_without_frontend());
  let body = json_get(&app, "/api/weather").await;
  let temp = body["temperature_c"].as_f64().unwrap();
  assert!((10.0..40.0).contains(&temp));
}

// ── zone CRUD route tests ───────────────────────────────────────────────────

async fn json_patch(
  app: &Router,
  uri: &str,
  body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
  let response = app
    .clone()
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap(),
    )
    .await
    .expect("request");
  let status = response.status();
  let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .expect("body");
  let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
  (status, value)
}

async fn json_delete(
  app: &Router,
  uri: &str,
) -> (StatusCode, serde_json::Value) {
  let response = app
    .clone()
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .expect("request");
  let status = response.status();
  let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .expect("body");
  let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
  (status, value)
}

#[tokio::test]
async fn test_create_zone_then_appears_in_property() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_post(
    &app,
    "/api/zones/definitions",
    serde_json::json!({
      "id": "zone-a4-new",
      "yard_id": "yard-a",
      "manifold_id": "manifold-a",
      "plant_kind": "shrub",
      "emitter_spec_id": "1gph-pc",
      "soil_type_id": "silty-clay-loam",
      "area_sq_ft": 80.0
    }),
  )
  .await;
  assert_eq!(status, StatusCode::OK, "create returned {body:?}");
  assert_eq!(body["id"], "zone-a4-new");

  let property = json_get(&app, "/api/sim/property").await;
  let ids: Vec<String> = property["zones"]
    .as_array()
    .unwrap()
    .iter()
    .map(|z| z["id"].as_str().unwrap().to_string())
    .collect();
  assert!(ids.contains(&"zone-a4-new".to_string()));

  let zones = json_get(&app, "/api/zones").await;
  assert_eq!(zones["zones"].as_array().unwrap().len(), 7);
}

#[tokio::test]
async fn test_create_zone_rejects_unknown_yard() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_post(
    &app,
    "/api/zones/definitions",
    serde_json::json!({
      "id": "zone-bad",
      "yard_id": "ghost-yard",
      "manifold_id": "manifold-a",
      "plant_kind": "shrub",
      "emitter_spec_id": "1gph-pc",
      "soil_type_id": "silty-clay-loam",
      "area_sq_ft": 80.0
    }),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert_eq!(body["kind"], "bad-request");
}

#[tokio::test]
async fn test_create_zone_rejects_unknown_catalog_ref() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_post(
    &app,
    "/api/zones/definitions",
    serde_json::json!({
      "id": "zone-bad",
      "yard_id": "yard-a",
      "manifold_id": "manifold-a",
      "plant_kind": "shrub",
      "emitter_spec_id": "ghost-emitter",
      "soil_type_id": "silty-clay-loam",
      "area_sq_ft": 80.0
    }),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert!(body["error"].as_str().unwrap().contains("emitter_spec_id"));
}

#[tokio::test]
async fn test_create_zone_rejects_duplicate_id() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_post(
    &app,
    "/api/zones/definitions",
    serde_json::json!({
      "id": "zone-a1-veggies",
      "yard_id": "yard-a",
      "manifold_id": "manifold-a",
      "plant_kind": "shrub",
      "emitter_spec_id": "1gph-pc",
      "soil_type_id": "silty-clay-loam",
      "area_sq_ft": 80.0
    }),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert!(body["error"].as_str().unwrap().contains("already exists"));
}

#[tokio::test]
async fn test_get_zone_definition() {
  let app = app_with_sim_routes(state_without_frontend());
  let body = json_get(&app, "/api/zones/definitions/zone-a1-veggies").await;
  assert_eq!(body["id"], "zone-a1-veggies");
  assert_eq!(body["plant_kind"], "veggie-bed");
  assert_eq!(body["yard_id"], "yard-a");
}

#[tokio::test]
async fn test_get_zone_definition_404() {
  let app = app_with_sim_routes(state_without_frontend());
  let response = app
    .oneshot(
      Request::builder()
        .uri("/api/zones/definitions/nope")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .expect("request");
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_patch_zone_updates_fields() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_patch(
    &app,
    "/api/zones/definitions/zone-a1-veggies",
    serde_json::json!({
      "area_sq_ft": 999.0,
      "notes": "new notes"
    }),
  )
  .await;
  assert_eq!(status, StatusCode::OK, "patch returned {body:?}");
  assert_eq!(body["area_sq_ft"], 999.0);
  assert_eq!(body["notes"], "new notes");
}

#[tokio::test]
async fn test_patch_zone_rejects_bad_catalog_ref() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_patch(
    &app,
    "/api/zones/definitions/zone-a1-veggies",
    serde_json::json!({"emitter_spec_id": "ghost"}),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert!(body["error"].as_str().unwrap().contains("emitter_spec_id"));
}

#[tokio::test]
async fn test_delete_zone_removes_it() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) =
    json_delete(&app, "/api/zones/definitions/zone-a1-veggies").await;
  assert_eq!(status, StatusCode::OK, "delete returned {body:?}");
  assert_eq!(body["zone_id"], "zone-a1-veggies");

  let property = json_get(&app, "/api/sim/property").await;
  let ids: Vec<String> = property["zones"]
    .as_array()
    .unwrap()
    .iter()
    .map(|z| z["id"].as_str().unwrap().to_string())
    .collect();
  assert!(!ids.contains(&"zone-a1-veggies".to_string()));

  let zones = json_get(&app, "/api/zones").await;
  assert_eq!(zones["zones"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn test_delete_unknown_zone_404() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, _body) = json_delete(&app, "/api/zones/definitions/ghost").await;
  assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── /api/plan tests ─────────────────────────────────────────────────────────

fn plan_req_body() -> serde_json::Value {
  serde_json::json!({
    "property_id": "test-property",
    "property_name": "Test Property",
    "climate_zone": "portland-or",
    "budget_usd": 1500.0,
    "prefer_smart_controller": true,
    "require_pressure_compensating": false,
    "soil_type_id": "silty-clay-loam",
    "top_n": 3,
    "yards": [{
      "id": "yard-a",
      "name": "Yard A",
      "area_sq_ft": 800.0,
      "mains_pressure_psi": 60.0,
      "zones": [
        {"name_suffix": "veggies", "plant_kind": "veggie-bed", "area_sq_ft": 100.0},
        {"name_suffix": "shrubs",  "plant_kind": "shrub",      "area_sq_ft": 200.0}
      ]
    }]
  })
}

#[tokio::test]
async fn test_plan_returns_ranked_candidates() {
  let app = app_with_sim_routes(state_without_frontend());
  let (status, body) = json_post(&app, "/api/plan", plan_req_body()).await;
  assert_eq!(status, StatusCode::OK);
  let plans = body["plans"].as_array().expect("plans array");
  assert!(!plans.is_empty(), "expected at least one plan");
  for plan in plans {
    assert!(plan["bom"]["total_usd"].as_f64().unwrap() > 0.0);
    assert!(plan["rationale"].as_array().unwrap().len() > 0);
    assert!(plan["controller_model_id"].is_string());
  }
}

#[tokio::test]
async fn test_plan_rejects_unknown_plant_kind() {
  let app = app_with_sim_routes(state_without_frontend());
  let mut body = plan_req_body();
  body["yards"][0]["zones"][0]["plant_kind"] =
    serde_json::Value::String("cactus-garden".into());
  let (status, _body) = json_post(&app, "/api/plan", body).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_plan_rejects_zero_zones() {
  let app = app_with_sim_routes(state_without_frontend());
  let mut body = plan_req_body();
  body["yards"][0]["zones"] = serde_json::Value::Array(Vec::new());
  let (status, _body) = json_post(&app, "/api/plan", body).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
}
