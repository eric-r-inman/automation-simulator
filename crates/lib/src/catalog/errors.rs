//! Semantic errors for catalog loading and validation.
//!
//! Each variant names the category and the offending id so a broken
//! catalog file reports the precise cause without the caller
//! having to parse the message.

use std::path::PathBuf;
use thiserror::Error;

use crate::sim::id::{
  ControllerModelId, EmitterSpecId, SensorModelId, SpeciesId,
};

#[derive(Debug, Error)]
pub enum CatalogLoadError {
  #[error("failed to read catalog file at {path:?} during load: {source}")]
  CatalogFileRead {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },

  #[error("failed to parse catalog file at {path:?}: {source}")]
  CatalogFileParse {
    path: PathBuf,
    #[source]
    source: toml::de::Error,
  },

  #[error(
    "catalog row in {path:?} has key {key:?} but its nested id \
     field reads {id:?}; the two must match"
  )]
  KeyIdMismatch {
    path: PathBuf,
    key: String,
    id: String,
  },

  #[error("duplicate controller model id {0} across catalog files")]
  DuplicateControllerModel(ControllerModelId),

  #[error("duplicate sensor model id {0} across catalog files")]
  DuplicateSensorModel(SensorModelId),

  #[error("duplicate emitter spec id {0} across catalog files")]
  DuplicateEmitterSpec(EmitterSpecId),

  #[error("duplicate species id {0} across catalog files")]
  DuplicateSpecies(SpeciesId),

  #[error(
    "sensor {sensor} declares gateway {gateway} but no sensor \
     model with that id exists in the catalog"
  )]
  UnknownSensorGateway {
    sensor: SensorModelId,
    gateway: SensorModelId,
  },

  #[error(
    "species {species} has invalid hardiness range \
     [{min}, {max}]; min must be ≤ max"
  )]
  InvalidHardinessRange {
    species: SpeciesId,
    min: i64,
    max: i64,
  },

  #[error(
    "soil type {0} has wilting_point_vwc ≥ field_capacity_vwc or \
     field_capacity_vwc ≥ saturation_vwc; the three must be \
     strictly increasing"
  )]
  NonMonotoneSoilVwc(crate::sim::id::SoilTypeId),

  #[error("pressure regulator {0} has input_psi_min ≥ input_psi_max")]
  InvalidRegulatorInputRange(crate::sim::id::PressureRegulatorModelId),

  #[error(
    "emitter spec {spec} of shape {shape:?} must have an inline \
     spacing set"
  )]
  InlineEmitterMissingSpacing {
    spec: EmitterSpecId,
    shape: crate::catalog::models::EmitterShape,
  },
}
