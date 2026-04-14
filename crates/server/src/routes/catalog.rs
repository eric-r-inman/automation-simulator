//! Catalog read endpoint for the dashboard's dropdowns.
//!
//! Returns the subset of the loaded catalog the frontend's zone
//! editor needs: emitter specs (with their typical pairings),
//! soil types, and species.  This deliberately omits the
//! controller / sensor / weather-station catalogs — those are
//! Phase 12 concerns and the v0.1 dashboard does not edit them
//! per zone.

use aide::axum::{routing::get_with, ApiRouter};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::Json;
use schemars::JsonSchema;
use serde::Serialize;

use crate::web_base::AppState;

#[derive(Debug, Serialize, JsonSchema)]
pub struct CatalogResponse {
  pub emitters: Vec<EmitterDto>,
  pub soil_types: Vec<SoilTypeDto>,
  pub species: Vec<SpeciesDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct EmitterDto {
  pub id: String,
  pub name: String,
  pub manufacturer: String,
  pub shape: String,
  pub flow_gph: f64,
  pub pressure_compensating: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SoilTypeDto {
  pub id: String,
  pub name: String,
  pub field_capacity_vwc: f64,
  pub wilting_point_vwc: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SpeciesDto {
  pub id: String,
  pub common_name: String,
  pub scientific_name: String,
  pub plant_kind: String,
  pub water_need_base_ml_per_day: f64,
}

async fn get_catalog(State(state): State<AppState>) -> Json<CatalogResponse> {
  let mut emitters: Vec<EmitterDto> = state
    .catalog
    .emitters
    .values()
    .map(|e| EmitterDto {
      id: e.id.as_str().to_string(),
      name: e.name.clone(),
      manufacturer: e.manufacturer.clone(),
      shape: format!("{:?}", e.shape).to_lowercase(),
      flow_gph: e.flow_gph,
      pressure_compensating: e.pressure_compensating,
    })
    .collect();
  emitters.sort_by(|a, b| a.id.cmp(&b.id));

  let mut soil_types: Vec<SoilTypeDto> = state
    .catalog
    .soil_types
    .values()
    .map(|s| SoilTypeDto {
      id: s.id.as_str().to_string(),
      name: s.name.clone(),
      field_capacity_vwc: s.field_capacity_vwc,
      wilting_point_vwc: s.wilting_point_vwc,
    })
    .collect();
  soil_types.sort_by(|a, b| a.id.cmp(&b.id));

  let mut species: Vec<SpeciesDto> = state
    .catalog
    .species
    .values()
    .map(|s| SpeciesDto {
      id: s.id.as_str().to_string(),
      common_name: s.common_name.clone(),
      scientific_name: s.scientific_name.clone(),
      plant_kind: format!("{:?}", s.kind).to_lowercase(),
      water_need_base_ml_per_day: s.water_need_base_ml_per_day,
    })
    .collect();
  species.sort_by(|a, b| a.id.cmp(&b.id));

  Json(CatalogResponse {
    emitters,
    soil_types,
    species,
  })
}

pub fn router() -> ApiRouter<AppState> {
  ApiRouter::new().api_route(
    "/api/catalog",
    get_with(get_catalog, |op: TransformOperation| {
      op.description(
        "Read-only subset of the loaded catalog: emitter specs, \
         soil types, and species.  Used by the dashboard's zone \
         editor to populate dropdowns.",
      )
    }),
  )
}
