//! `automation-simulator-cli seed` — load a property fixture.
//!
//! Reads the catalog directory, validates the property TOML
//! against it, opens (creating if needed) the target SQLite
//! database, runs migrations, and inserts the validated bundle.
//! Errors propagate with full context via `SeedError`.

use std::path::Path;

use automation_simulator_lib::{
  catalog::{Catalog, CatalogLoadError},
  db::{DbOpenError, SimDb},
  seed::{seed_property, SeedError},
};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum SeedCommandError {
  #[error("failed to load catalog from {path:?}: {source}")]
  Catalog {
    path: std::path::PathBuf,
    #[source]
    source: CatalogLoadError,
  },

  #[error("failed to open database: {0}")]
  OpenDb(#[from] DbOpenError),

  #[error("seed failed: {0}")]
  Seed(#[from] SeedError),
}

pub async fn run(
  property: &Path,
  catalog_dir: &Path,
  db_path: &Path,
) -> Result<(), SeedCommandError> {
  info!(catalog = %catalog_dir.display(), "Loading catalog");
  let catalog =
    Catalog::load(catalog_dir).map_err(|source| SeedCommandError::Catalog {
      path: catalog_dir.to_path_buf(),
      source,
    })?;
  info!(
    controllers = catalog.controllers.len(),
    sensors = catalog.sensors.len(),
    species = catalog.species.len(),
    soil_types = catalog.soil_types.len(),
    "Catalog loaded"
  );

  info!(db = %db_path.display(), "Opening database");
  let db = SimDb::connect(db_path).await?;

  info!(property = %property.display(), "Seeding property");
  let bundle = seed_property(property, &catalog, &db).await?;
  info!(
    property_id = %bundle.property.id,
    yards = bundle.property.yards.len(),
    spigots = bundle.property.spigots.len(),
    zones = bundle.zones.len(),
    plants = bundle.plants.len(),
    controllers = bundle.controllers.len(),
    sensors = bundle.sensors.len(),
    "Seed complete"
  );
  Ok(())
}
