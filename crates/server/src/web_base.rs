use aide::{
  axum::{routing::get_with, ApiRouter},
  openapi::OpenApi,
  scalar::Scalar,
  transform::TransformOperation,
};
use automation_simulator_lib::{
  catalog::{Catalog, CatalogLoadError},
  engine::SimWorld,
  hw::{
    Controller, SensorSource, SharedWorld, SimulatedController,
    SimulatedSensorSource,
  },
  seed::{load_property, PropertyBundle, SeedError},
};
use axum::{
  http::{header, HeaderValue, StatusCode},
  response::{IntoResponse, Response},
  routing::get,
  Json, Router,
};
use openidconnect::core::CoreClient;
use prometheus::{Encoder, IntCounter, Registry, TextEncoder};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tower::ServiceBuilder;
use tower_http::{
  services::{ServeDir, ServeFile},
  set_header::SetResponseHeaderLayer,
};
use tracing::info;

use crate::{auth, config::Config};

// ── AppState ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
  pub registry: Arc<Registry>,
  pub request_counter: IntCounter,
  pub frontend_path: PathBuf,
  pub oidc_client: Option<Arc<CoreClient>>,
  /// Shared reference to the simulator world.  Routes that need to
  /// read *or* mutate state beyond what the trait surface exposes
  /// (e.g. `POST /api/sim/step` advances the clock, which is a
  /// simulator-only concept) hold `SharedWorld` directly.
  pub world: SharedWorld,
  /// Controller trait object.  Routes under `/api/zones/*` talk to
  /// this so a future `OpenSprinklerController` drops in without
  /// touching them.
  pub controller: Arc<dyn Controller>,
  /// Sensor-source trait object.  Routes under `/api/sensors/*`
  /// and `/api/weather` use this.
  pub sensors: Arc<dyn SensorSource>,
  /// The catalog that backed the world.  Served as JSON at
  /// `/api/catalog` in v0.3 so the Designer UI knows what hardware
  /// is available; kept here so each request does not re-parse the
  /// TOML.
  pub catalog: Arc<Catalog>,
  /// Validated property bundle loaded at startup.  Wrapped in a
  /// tokio Mutex so the zone-CRUD routes can mutate the property's
  /// zone roster atomically with the corresponding `SimWorld`
  /// update.  Reads (e.g. `GET /api/sim/property`) lock briefly,
  /// clone, drop the lock — never hold across `.await`.
  pub property: Arc<tokio::sync::Mutex<PropertyBundle>>,
}

#[derive(Debug, Error)]
pub enum AppStateError {
  #[error("Invalid OIDC issuer URL: {0}")]
  InvalidIssuer(String),

  #[error("OIDC provider discovery failed: {0}")]
  OidcDiscovery(String),

  #[error("Invalid OIDC redirect URI: {0}")]
  InvalidRedirectUri(String),

  #[error("failed to load hardware + species catalog from {path:?}: {source}")]
  CatalogLoad {
    path: PathBuf,
    #[source]
    source: CatalogLoadError,
  },

  #[error("failed to load property fixture from {path:?}: {source}")]
  PropertyLoad {
    path: PathBuf,
    #[source]
    source: SeedError,
  },

  #[error("failed to build the simulated world from the property: {0}")]
  WorldBuild(String),
}

impl AppState {
  pub fn auth_enabled(&self) -> bool {
    self.oidc_client.is_some()
  }

  /// Construct `AppState` from a validated `Config`.
  ///
  /// Performs OIDC discovery when OIDC is configured (an async HTTP call).
  pub async fn init(config: &Config) -> Result<Self, AppStateError> {
    let registry = Registry::new();
    let request_counter =
      IntCounter::new("http_requests_total", "Total HTTP requests")
        .expect("Failed to create counter");
    registry
      .register(Box::new(request_counter.clone()))
      .expect("Failed to register counter");

    let oidc_client = match &config.oidc {
      Some(oidc) => {
        let issuer = openidconnect::IssuerUrl::new(oidc.issuer.clone())
          .map_err(|e| AppStateError::InvalidIssuer(e.to_string()))?;

        let provider_metadata =
          openidconnect::core::CoreProviderMetadata::discover_async(
            issuer,
            openidconnect::reqwest::async_http_client,
          )
          .await
          .map_err(|e| AppStateError::OidcDiscovery(format!("{e:?}")))?;

        info!(issuer = %oidc.issuer, "OIDC discovery complete");

        let redirect_url = openidconnect::RedirectUrl::new(format!(
          "{}/auth/callback",
          config.base_url.trim_end_matches('/')
        ))
        .map_err(|e| AppStateError::InvalidRedirectUri(e.to_string()))?;

        // RequestBody sends client credentials in the POST body
        // (client_secret_post).  Some providers (e.g. Authelia) require this
        // instead of the HTTP Basic Auth default.
        let client = openidconnect::core::CoreClient::from_provider_metadata(
          provider_metadata,
          openidconnect::ClientId::new(oidc.client_id.clone()),
          Some(openidconnect::ClientSecret::new(oidc.client_secret.clone())),
        )
        .set_redirect_uri(redirect_url)
        .set_auth_type(openidconnect::AuthType::RequestBody);

        Some(Arc::new(client))
      }
      None => {
        info!("OIDC not configured — running unauthenticated");
        None
      }
    };

    // Simulator boot: load catalog, load property fixture, validate
    // against the catalog, build SimWorld + the Simulated* impls.
    // Any failure here stops the server from starting — we prefer a
    // loud startup error over serving a broken /api/sim/* route.
    let catalog = Catalog::load(&config.catalog_path).map_err(|source| {
      AppStateError::CatalogLoad {
        path: config.catalog_path.clone(),
        source,
      }
    })?;
    let catalog = Arc::new(catalog);

    let bundle =
      load_property(&config.property_path, &catalog).map_err(|source| {
        AppStateError::PropertyLoad {
          path: config.property_path.clone(),
          source,
        }
      })?;

    info!(
      property_id = %bundle.property.id,
      zones = bundle.zones.len(),
      plants = bundle.plants.len(),
      "Property fixture loaded"
    );

    // Rebuild the world's zones vector from the validated bundle so
    // `SimWorld` sees them in the fixture's stable order.
    let zones = bundle.zones.clone();
    let sim_world = SimWorld::new(
      // TODO: scenario start-date and seed will become CLI/config
      // inputs in Phase 9; for now hard-code a sensible default that
      // puts the simulator in mid-summer Portland so the dashboard
      // shows interesting weather out of the box.
      chrono::NaiveDate::from_ymd_opt(2026, 7, 1).expect("valid date"),
      &bundle.property.climate_zone,
      zones,
      Arc::clone(&catalog),
      1,
      0.30,
      Vec::new(),
    )
    .map_err(|e| AppStateError::WorldBuild(e.to_string()))?;

    let world = SharedWorld::new(sim_world);
    let controller: Arc<dyn Controller> =
      Arc::new(SimulatedController::new(world.clone()));
    let sensors: Arc<dyn SensorSource> =
      Arc::new(SimulatedSensorSource::new(world.clone()));

    Ok(Self {
      registry: Arc::new(registry),
      request_counter,
      frontend_path: config.frontend_path.clone(),
      oidc_client,
      world,
      controller,
      sensors,
      catalog,
      property: Arc::new(tokio::sync::Mutex::new(bundle)),
    })
  }
}

// ── base router ───────────────────────────────────────────────────────────────

#[derive(Serialize, JsonSchema)]
pub struct HealthResponse {
  status: String,
}

async fn healthz() -> Json<HealthResponse> {
  Json(HealthResponse {
    status: "healthy".to_string(),
  })
}

#[derive(Serialize, JsonSchema)]
pub struct MeResponse {
  name: String,
  auth_enabled: bool,
}

async fn me_handler(
  axum::extract::State(state): axum::extract::State<AppState>,
  session: tower_sessions::Session,
) -> Json<MeResponse> {
  if !state.auth_enabled() {
    return Json(MeResponse {
      name: "admin".to_string(),
      auth_enabled: false,
    });
  }

  let name = auth::current_user(&session)
    .await
    .map(|u| u.name)
    .unwrap_or_else(|| "anonymous".to_string());

  Json(MeResponse {
    name,
    auth_enabled: true,
  })
}

pub fn base_router(state: AppState) -> Router {
  aide::generate::extract_schemas(true);
  let frontend_path = state.frontend_path.clone();
  let me_state = state.clone();
  let mut api = OpenApi::default();

  let app_router = ApiRouter::new()
    .api_route(
      "/healthz",
      get_with(healthz, |op: TransformOperation| {
        op.description("Health check.")
      }),
    )
    .api_route(
      "/metrics",
      get_with(metrics_endpoint, |op: TransformOperation| {
        op.description("Prometheus metrics in text/plain format.")
      }),
    )
    .with_state(state)
    .finish_api_with(&mut api, |a| a.title("automation-simulator"));

  let api = Arc::new(api);

  Router::new()
    .merge(app_router)
    .route("/me", get(me_handler).with_state(me_state))
    .route(
      "/api-docs/openapi.json",
      get({
        let api = api.clone();
        move || async move { Json((*api).clone()) }
      }),
    )
    .route(
      "/scalar",
      get(
        Scalar::new("/api-docs/openapi.json")
          .with_title("automation-simulator")
          .axum_handler(),
      ),
    )
    .fallback_service(
      ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
          header::CACHE_CONTROL,
          HeaderValue::from_static("no-store"),
        ))
        .service(
          ServeDir::new(&frontend_path)
            .fallback(ServeFile::new(frontend_path.join("index.html"))),
        ),
    )
}

async fn metrics_endpoint(
  axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
  let encoder = TextEncoder::new();
  let metric_families = state.registry.gather();
  let mut buffer = Vec::new();

  match encoder.encode(&metric_families, &mut buffer) {
    Ok(_) => {
      (StatusCode::OK, [("content-type", encoder.format_type())], buffer)
        .into_response()
    }
    Err(e) => (
      StatusCode::INTERNAL_SERVER_ERROR,
      Json(json!({
          "error": format!("Failed to encode metrics: {}", e)
      })),
    )
      .into_response(),
  }
}
