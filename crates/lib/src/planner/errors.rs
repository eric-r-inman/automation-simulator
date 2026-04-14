//! Semantic errors for the recommender.  Each variant names what
//! the planner couldn't satisfy so the UI can give a precise hint
//! ("no controller can handle 28 zones — split into multiple
//! properties or upgrade your hardware shortlist").

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlannerError {
  #[error("requirements ask for 0 zones across all yards; nothing to plan")]
  NoZonesRequested,

  #[error(
    "no controller in the catalog can handle {requested} zones \
     (largest available is {max_available})"
  )]
  NoControllerLargeEnough {
    requested: usize,
    max_available: i64,
  },

  #[error(
    "yard {yard} requests {requested} zones but no manifold can cover \
     that many (largest available is {max_available})"
  )]
  NoManifoldLargeEnough {
    yard: String,
    requested: usize,
    max_available: i64,
  },

  #[error(
    "no emitter in the catalog matches plant kind {plant_kind:?} \
     under the requirements (pressure-compensating: {pc_required})"
  )]
  NoEmitterForPlantKind {
    plant_kind: crate::sim::zone::PlantKind,
    pc_required: bool,
  },

  #[error(
    "no pressure regulator can serve mains pressure {mains_psi} psi \
     (input range required)"
  )]
  NoPressureRegulator { mains_psi: f64 },

  #[error("catalog has no backflow preventer — at least one is required")]
  NoBackflowPreventer,

  #[error("catalog has no soil type {soil_type:?}")]
  UnknownSoilType { soil_type: String },

  #[error(
    "the candidate plan failed seed validation, which means the \
     planner produced an internally-inconsistent property: {0}"
  )]
  PlanInternallyInvalid(#[from] crate::seed::SeedError),
}
