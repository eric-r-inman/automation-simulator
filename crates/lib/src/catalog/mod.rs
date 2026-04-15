//! Hardware, species, and soil-type catalog.
//!
//! The catalog is the single source of truth for datasheet
//! constants.  Fixture files reference catalog rows by id; the
//! simulation engine reads physical constants through the catalog.
//! Growing support for new hardware in v0.3 is purely a data change:
//! add a row to `data/catalog/*.toml`, the domain model and engine
//! remain untouched.
//!
//! The loader pattern is deliberately strict.  Every file under
//! `data/catalog/` parses into a fixed TOML shape — a top-level
//! table whose keys are ids and whose values are rows.  The key must
//! equal the row's `id` field.  Cross-category references (like a
//! sensor naming its gateway) are resolved after every file has
//! parsed, so an unknown id is always reported with the file that
//! introduced it rather than the file that defined it.

pub mod errors;
pub mod models;

pub use errors::CatalogLoadError;
pub use models::{
  BackflowKind, BackflowPreventerModel, ComputeHostModel, ControllerModel,
  DripLineModel, EmitterShape, EmitterSpec, ManifoldModel,
  PressureRegulatorModel, SensorKind, SensorModel, SoilType, Species,
  ValveModel, WeatherStationModel,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::sim::id::{
  BackflowPreventerModelId, ComputeHostModelId, ControllerModelId,
  DripLineModelId, EmitterSpecId, ManifoldModelId, PressureRegulatorModelId,
  SensorModelId, SoilTypeId, SpeciesId, ValveModelId, WeatherStationModelId,
};

/// Convenience envelope for a single TOML file whose top-level
/// table maps ids to rows of type `T`.  All catalog files share
/// this shape; only the value type differs per category.
#[derive(Debug, Deserialize, Serialize)]
struct CategoryFile<T> {
  #[serde(flatten)]
  rows: HashMap<String, T>,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
  pub controllers: HashMap<ControllerModelId, ControllerModel>,
  pub sensors: HashMap<SensorModelId, SensorModel>,
  pub weather_stations: HashMap<WeatherStationModelId, WeatherStationModel>,
  pub manifolds: HashMap<ManifoldModelId, ManifoldModel>,
  pub valves: HashMap<ValveModelId, ValveModel>,
  pub emitters: HashMap<EmitterSpecId, EmitterSpec>,
  pub pressure_regulators:
    HashMap<PressureRegulatorModelId, PressureRegulatorModel>,
  pub backflow_preventers:
    HashMap<BackflowPreventerModelId, BackflowPreventerModel>,
  pub drip_lines: HashMap<DripLineModelId, DripLineModel>,
  pub compute_hosts: HashMap<ComputeHostModelId, ComputeHostModel>,
  pub species: HashMap<SpeciesId, Species>,
  pub soil_types: HashMap<SoilTypeId, SoilType>,
}

impl Catalog {
  /// Load every `*.toml` under `dir` into a fresh catalog.  The
  /// filename determines the category: `controllers.toml`,
  /// `sensors.toml`, and so on.  Files for categories the caller
  /// does not use may be absent; files with unrecognized names are
  /// ignored so the directory may hold arbitrary notes alongside
  /// the catalog.
  pub fn load(dir: impl AsRef<Path>) -> Result<Self, CatalogLoadError> {
    let dir = dir.as_ref();
    let mut cat = Catalog::default();

    cat.controllers =
      load_category(dir, "controllers.toml", |row: &ControllerModel| {
        row.id.clone()
      })?;
    cat.sensors =
      load_category(dir, "sensors.toml", |row: &SensorModel| row.id.clone())?;
    cat.weather_stations = load_category(
      dir,
      "weather-stations.toml",
      |row: &WeatherStationModel| row.id.clone(),
    )?;
    cat.manifolds =
      load_category(dir, "manifolds.toml", |row: &ManifoldModel| {
        row.id.clone()
      })?;
    cat.valves =
      load_category(dir, "valves.toml", |row: &ValveModel| row.id.clone())?;
    cat.emitters =
      load_category(dir, "emitters.toml", |row: &EmitterSpec| row.id.clone())?;
    cat.pressure_regulators = load_category(
      dir,
      "pressure-regulators.toml",
      |row: &PressureRegulatorModel| row.id.clone(),
    )?;
    cat.backflow_preventers = load_category(
      dir,
      "backflow-preventers.toml",
      |row: &BackflowPreventerModel| row.id.clone(),
    )?;
    cat.drip_lines =
      load_category(dir, "drip-lines.toml", |row: &DripLineModel| {
        row.id.clone()
      })?;
    cat.compute_hosts =
      load_category(dir, "compute-hosts.toml", |row: &ComputeHostModel| {
        row.id.clone()
      })?;
    cat.species =
      load_category(dir, "species.toml", |row: &Species| row.id.clone())?;
    cat.soil_types =
      load_category(dir, "soil-types.toml", |row: &SoilType| row.id.clone())?;

    cat.validate()?;
    Ok(cat)
  }

  /// Run the structural and cross-reference invariants that every
  /// loaded catalog must satisfy.
  fn validate(&self) -> Result<(), CatalogLoadError> {
    for sensor in self.sensors.values() {
      if let Some(gw) = &sensor.gateway_model_id {
        if !self.sensors.contains_key(gw) {
          return Err(CatalogLoadError::UnknownSensorGateway {
            sensor: sensor.id.clone(),
            gateway: gw.clone(),
          });
        }
      }
    }

    for s in self.species.values() {
      if s.hardiness_zone_min > s.hardiness_zone_max {
        return Err(CatalogLoadError::InvalidHardinessRange {
          species: s.id.clone(),
          min: s.hardiness_zone_min,
          max: s.hardiness_zone_max,
        });
      }
    }

    for st in self.soil_types.values() {
      let ok = st.wilting_point_vwc < st.field_capacity_vwc
        && st.field_capacity_vwc < st.saturation_vwc;
      if !ok {
        return Err(CatalogLoadError::NonMonotoneSoilVwc(st.id.clone()));
      }
    }

    for reg in self.pressure_regulators.values() {
      if reg.input_psi_min >= reg.input_psi_max {
        return Err(CatalogLoadError::InvalidRegulatorInputRange(
          reg.id.clone(),
        ));
      }
    }

    for emitter in self.emitters.values() {
      if emitter.shape == EmitterShape::InlineDrip
        && emitter.inline_spacing_inches.is_none()
      {
        return Err(CatalogLoadError::InlineEmitterMissingSpacing {
          spec: emitter.id.clone(),
          shape: emitter.shape,
        });
      }
    }

    Ok(())
  }
}

/// Helper trait exposing a row's id as a display string, for the
/// key-vs-id consistency check in `load_category`.
pub(crate) trait HasId {
  fn id_string(&self) -> String;
}

macro_rules! impl_has_id {
  ($model:ty) => {
    impl HasId for $model {
      fn id_string(&self) -> String {
        self.id.to_string()
      }
    }
  };
}

impl_has_id!(ControllerModel);
impl_has_id!(SensorModel);
impl_has_id!(WeatherStationModel);
impl_has_id!(ManifoldModel);
impl_has_id!(ValveModel);
impl_has_id!(EmitterSpec);
impl_has_id!(PressureRegulatorModel);
impl_has_id!(BackflowPreventerModel);
impl_has_id!(DripLineModel);
impl_has_id!(ComputeHostModel);
impl_has_id!(Species);
impl_has_id!(SoilType);

/// Load one category TOML file.  Returns an empty map when the file
/// does not exist — that is how a caller opts out of a category.
fn load_category<T, I, F>(
  dir: &Path,
  filename: &str,
  key_of: F,
) -> Result<HashMap<I, T>, CatalogLoadError>
where
  T: for<'de> Deserialize<'de> + HasId,
  I: std::hash::Hash + Eq + Clone,
  F: Fn(&T) -> I,
{
  let path = dir.join(filename);
  if !path.exists() {
    return Ok(HashMap::new());
  }

  let contents = std::fs::read_to_string(&path).map_err(|source| {
    CatalogLoadError::CatalogFileRead {
      path: path.clone(),
      source,
    }
  })?;

  let parsed: CategoryFile<T> =
    toml::from_str(&contents).map_err(|source| {
      CatalogLoadError::CatalogFileParse {
        path: path.clone(),
        source,
      }
    })?;

  let mut out: HashMap<I, T> = HashMap::with_capacity(parsed.rows.len());
  for (key_str, row) in parsed.rows {
    let row_id = row.id_string();
    if row_id != key_str {
      return Err(CatalogLoadError::KeyIdMismatch {
        path: path.clone(),
        key: key_str,
        id: row_id,
      });
    }
    let id = key_of(&row);
    out.insert(id, row);
  }
  Ok(out)
}

#[cfg(test)]
mod tests;
