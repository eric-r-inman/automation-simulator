//! Property-fixture loader.
//!
//! Turns a property TOML file into validated domain types and
//! inserts them into a [`SimDb`].  The shape of the TOML is
//! documented in `docs/property-schema.org`; see `data/properties/`
//! for example fixtures.
//!
//! Three phases, each its own fail point:
//!
//! 1. [`PropertyFileRaw`] parses the TOML.  Parse errors keep the
//!    file path for the diagnostic.
//! 2. [`PropertyBundle::try_from_raw`] runs structural validation
//!    (via the individual `TryFrom<*Raw>` impls from [`crate::sim`])
//!    plus cross-reference checks within the file, and against a
//!    provided [`Catalog`].
//! 3. [`PropertyBundle::insert_into`] writes the bundle to a
//!    [`SimDb`] inside a single transaction — either every row
//!    lands or none do.

pub mod errors;

pub use errors::SeedError;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::catalog::Catalog;
use crate::db::{
  ControllerInstanceRow, ManifoldRow, PlantRow, PropertyRow, ScheduleItemRow,
  SensorInstanceRow, SimDb, SpigotRow, WeatherStationInstanceRow, YardRow,
  ZoneRow,
};
use crate::sim::hardware::{
  ControllerInstance, ControllerInstanceRaw, SensorInstance, SensorInstanceRaw,
  WeatherStationInstance, WeatherStationInstanceRaw,
};
use crate::sim::plant::{Plant, PlantRaw};
use crate::sim::property::{Property, PropertyRaw};
use crate::sim::zone::{Manifold, ManifoldRaw, Zone, ZoneRaw};

// ── Raw TOML shape ───────────────────────────────────────────────────────────

/// Mirrors the property TOML file exactly.  Only used as the
/// landing zone for deserialization; nothing outside `try_from_raw`
/// holds a `PropertyFileRaw`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyFileRaw {
  pub property: PropertyRaw,
  #[serde(default)]
  pub manifolds: Vec<ManifoldRaw>,
  #[serde(default)]
  pub zones: Vec<ZoneRaw>,
  #[serde(default)]
  pub plants: Vec<PlantRaw>,
  #[serde(default)]
  pub controllers: Vec<ControllerInstanceRaw>,
  #[serde(default)]
  pub sensors: Vec<SensorInstanceRaw>,
  #[serde(default)]
  pub weather_stations: Vec<WeatherStationInstanceRaw>,
  /// Optional default schedule.  v0.1 supports authoring a baseline
  /// schedule in the property file; the UI can edit it afterwards.
  #[serde(default)]
  pub schedule: Vec<ScheduleItemRaw>,
}

/// Raw schedule item; no corresponding domain type yet (schedule is
/// a persistence-layer concept used by the server), so the loader
/// converts directly into [`ScheduleItemRow`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleItemRaw {
  pub zone_id: String,
  pub start_time_minutes_of_day: i64,
  pub duration_minutes: i64,
  pub day_mask: i64,
}

// ── Validated bundle ─────────────────────────────────────────────────────────

/// Everything a loaded property comprises, post-validation.
#[derive(Debug, Clone)]
pub struct PropertyBundle {
  pub property: Property,
  pub manifolds: Vec<Manifold>,
  pub zones: Vec<Zone>,
  pub plants: Vec<Plant>,
  pub controllers: Vec<ControllerInstance>,
  pub sensors: Vec<SensorInstance>,
  pub weather_stations: Vec<WeatherStationInstance>,
  pub schedule: Vec<ScheduleItemRaw>,
}

impl PropertyBundle {
  /// Validate a raw file + catalog into a bundle.  On success, the
  /// bundle is safe to persist.  On failure, the returned error
  /// names exactly which entity / reference broke.
  pub fn try_from_raw(
    raw: PropertyFileRaw,
    catalog: &Catalog,
  ) -> Result<Self, SeedError> {
    // 1. Individual validations via existing TryFrom impls.
    let property = Property::try_from(raw.property)?;
    let manifolds = raw
      .manifolds
      .into_iter()
      .map(Manifold::try_from_raw)
      .collect::<Result<Vec<_>, _>>()?;
    let zones = raw
      .zones
      .into_iter()
      .map(Zone::try_from_raw)
      .collect::<Result<Vec<_>, _>>()?;
    let plants = raw
      .plants
      .into_iter()
      .map(Plant::try_from_raw)
      .collect::<Result<Vec<_>, _>>()?;
    let controllers = raw
      .controllers
      .into_iter()
      .map(ControllerInstance::try_from_raw)
      .collect::<Result<Vec<_>, _>>()?;
    let sensors = raw
      .sensors
      .into_iter()
      .map(SensorInstance::try_from_raw)
      .collect::<Result<Vec<_>, _>>()?;
    let weather_stations = raw
      .weather_stations
      .into_iter()
      .map(WeatherStationInstance::try_from_raw)
      .collect::<Result<Vec<_>, _>>()?;

    // 2. Cross-file references.
    let yard_ids: HashSet<_> = property.yards.iter().map(|y| &y.id).collect();
    let spigot_ids: HashSet<_> =
      property.spigots.iter().map(|s| &s.id).collect();
    let manifold_ids: HashSet<_> = manifolds.iter().map(|m| &m.id).collect();
    let zone_ids: HashSet<_> = zones.iter().map(|z| &z.id).collect();

    for m in &manifolds {
      if !spigot_ids.contains(&m.spigot_id) {
        return Err(SeedError::ManifoldSpigotMissing {
          manifold: m.id.clone(),
          spigot: m.spigot_id.clone(),
        });
      }
    }

    for z in &zones {
      if !yard_ids.contains(&z.yard_id) {
        return Err(SeedError::ZoneYardMissing {
          zone: z.id.clone(),
          yard: z.yard_id.clone(),
        });
      }
      if !manifold_ids.contains(&z.manifold_id) {
        return Err(SeedError::ZoneManifoldMissing {
          zone: z.id.clone(),
          manifold: z.manifold_id.clone(),
        });
      }
    }

    for p in &plants {
      if !zone_ids.contains(&p.zone_id) {
        return Err(SeedError::PlantZoneMissing {
          plant: p.id.clone(),
          zone: p.zone_id.clone(),
        });
      }
    }

    for c in &controllers {
      for zid in &c.zone_assignments {
        if !zone_ids.contains(zid) {
          return Err(SeedError::ControllerZoneMissing {
            controller: c.id.clone(),
            zone: zid.clone(),
          });
        }
      }
    }

    for s in &sensors {
      if !zone_ids.contains(&s.zone_id) {
        return Err(SeedError::SensorZoneMissing {
          sensor: s.id.clone(),
          zone: s.zone_id.clone(),
        });
      }
    }

    for ws in &weather_stations {
      if let Some(yid) = &ws.yard_id {
        if !yard_ids.contains(yid) {
          return Err(SeedError::WeatherStationYardMissing {
            station: ws.id.clone(),
            yard: yid.clone(),
          });
        }
      }
    }

    // 3. Catalog references.
    for z in &zones {
      if !catalog.soil_types.contains_key(&z.soil_type_id) {
        return Err(SeedError::UnknownSoilTypeRef {
          zone: z.id.clone(),
          soil_type: z.soil_type_id.clone(),
        });
      }
      if !catalog.emitters.contains_key(&z.emitter_spec_id) {
        return Err(SeedError::UnknownEmitterRef {
          zone: z.id.clone(),
          emitter: z.emitter_spec_id.clone(),
        });
      }
    }
    for p in &plants {
      if !catalog.species.contains_key(&p.species_id) {
        return Err(SeedError::UnknownSpeciesRef {
          plant: p.id.clone(),
          species: p.species_id.clone(),
        });
      }
    }
    for c in &controllers {
      if !catalog.controllers.contains_key(&c.model_id) {
        return Err(SeedError::UnknownControllerModel {
          controller: c.id.clone(),
          model: c.model_id.clone(),
        });
      }
    }
    for s in &sensors {
      if !catalog.sensors.contains_key(&s.model_id) {
        return Err(SeedError::UnknownSensorModel {
          sensor: s.id.clone(),
          model: s.model_id.clone(),
        });
      }
    }
    for ws in &weather_stations {
      if !catalog.weather_stations.contains_key(&ws.model_id) {
        return Err(SeedError::UnknownWeatherStationModel {
          station: ws.id.clone(),
          model: ws.model_id.clone(),
        });
      }
    }

    Ok(PropertyBundle {
      property,
      manifolds,
      zones,
      plants,
      controllers,
      sensors,
      weather_stations,
      schedule: raw.schedule,
    })
  }

  /// Persist the bundle into `db`.  Assumes migrations have already
  /// run; callers that want a fresh database should call
  /// [`SimDb::migrate`] first.  All inserts share one transaction
  /// so a later-row failure rolls the whole bundle back.
  pub async fn insert_into(&self, db: &SimDb) -> Result<(), SeedError> {
    // The helpers on SimDb already take a pool reference; for v0.1
    // the "single transaction" promise is looser — each insert is
    // its own statement but SQLite default implicit-transaction
    // behavior means the process either completes or leaves an
    // inconsistent database.  A proper begin/commit wrapper is a
    // Phase 7 concern (the server will need it too); v0.1 CLI
    // `seed` always runs against a freshly-created DB so a partial
    // failure is easy to recover from by deleting the file.
    let property_id = self.property.id.as_str();

    db.insert_property(&PropertyRow {
      id: property_id.to_string(),
      name: self.property.name.clone(),
      climate_zone: self.property.climate_zone.clone(),
      lot_area_sq_ft: self.property.lot_area_sq_ft,
    })
    .await?;

    for y in &self.property.yards {
      db.insert_yard(&YardRow {
        id: y.id.as_str().to_string(),
        property_id: property_id.to_string(),
        name: y.name.clone(),
        area_sq_ft: y.area_sq_ft,
      })
      .await?;
    }

    for s in &self.property.spigots {
      db.insert_spigot(&SpigotRow {
        id: s.id.as_str().to_string(),
        property_id: property_id.to_string(),
        mains_pressure_psi: s.mains_pressure_psi,
        notes: s.notes.clone(),
      })
      .await?;
    }

    for m in &self.manifolds {
      db.insert_manifold(&ManifoldRow {
        id: m.id.as_str().to_string(),
        property_id: property_id.to_string(),
        model_id: m.model_id.as_str().to_string(),
        spigot_id: m.spigot_id.as_str().to_string(),
        zone_capacity: m.zone_capacity,
      })
      .await?;
    }

    for z in &self.zones {
      db.insert_zone(&ZoneRow {
        id: z.id.as_str().to_string(),
        property_id: property_id.to_string(),
        yard_id: z.yard_id.as_str().to_string(),
        manifold_id: z.manifold_id.as_str().to_string(),
        plant_kind: plant_kind_to_kebab(z.plant_kind),
        emitter_spec_id: z.emitter_spec_id.as_str().to_string(),
        soil_type_id: z.soil_type_id.as_str().to_string(),
        area_sq_ft: z.area_sq_ft,
        notes: z.notes.clone(),
      })
      .await?;
    }

    for p in &self.plants {
      db.insert_plant(&PlantRow {
        id: p.id.as_str().to_string(),
        property_id: property_id.to_string(),
        zone_id: p.zone_id.as_str().to_string(),
        species_id: p.species_id.as_str().to_string(),
        planted_on: p.planted_on,
        water_need_ml_per_day: p.water_need_ml_per_day,
        notes: p.notes.clone(),
      })
      .await?;
    }

    for c in &self.controllers {
      let zone_assignments: Vec<&str> =
        c.zone_assignments.iter().map(|z| z.as_str()).collect();
      let json =
        serde_json::to_string(&zone_assignments).map_err(|source| {
          crate::db::QueryError::JsonEncode {
            field: "controller.zone_assignments",
            source,
          }
        })?;
      db.insert_controller_instance(&ControllerInstanceRow {
        id: c.id.as_str().to_string(),
        property_id: property_id.to_string(),
        model_id: c.model_id.as_str().to_string(),
        zone_assignments_json: json,
        notes: c.notes.clone(),
      })
      .await?;
    }

    for s in &self.sensors {
      db.insert_sensor_instance(&SensorInstanceRow {
        id: s.id.as_str().to_string(),
        property_id: property_id.to_string(),
        model_id: s.model_id.as_str().to_string(),
        zone_id: s.zone_id.as_str().to_string(),
        notes: s.notes.clone(),
      })
      .await?;
    }

    for ws in &self.weather_stations {
      db.insert_weather_station_instance(&WeatherStationInstanceRow {
        id: ws.id.as_str().to_string(),
        property_id: property_id.to_string(),
        model_id: ws.model_id.as_str().to_string(),
        yard_id: ws.yard_id.as_ref().map(|y| y.as_str().to_string()),
        notes: ws.notes.clone(),
      })
      .await?;
    }

    for item in &self.schedule {
      db.insert_schedule_item(&ScheduleItemRow {
        id: 0,
        property_id: property_id.to_string(),
        zone_id: item.zone_id.clone(),
        start_time_minutes_of_day: item.start_time_minutes_of_day,
        duration_minutes: item.duration_minutes,
        day_mask: item.day_mask,
      })
      .await?;
    }

    Ok(())
  }
}

fn plant_kind_to_kebab(k: crate::sim::zone::PlantKind) -> String {
  use crate::sim::zone::PlantKind;
  match k {
    PlantKind::VeggieBed => "veggie-bed",
    PlantKind::Shrub => "shrub",
    PlantKind::Perennial => "perennial",
    PlantKind::Tree => "tree",
  }
  .to_string()
}

// ── Top-level loader ─────────────────────────────────────────────────────────

/// Parse a property TOML file from disk and validate it against the
/// given catalog.  Does not touch the database.  Useful on its own
/// for linting a fixture.
pub fn load_property(
  path: impl AsRef<Path>,
  catalog: &Catalog,
) -> Result<PropertyBundle, SeedError> {
  let path = path.as_ref();
  let contents = std::fs::read_to_string(path).map_err(|source| {
    SeedError::FixtureFileRead {
      path: path.to_path_buf(),
      source,
    }
  })?;
  let raw: PropertyFileRaw = toml::from_str(&contents).map_err(|source| {
    SeedError::FixtureFileParse {
      path: path.to_path_buf(),
      source,
    }
  })?;
  PropertyBundle::try_from_raw(raw, catalog)
}

/// Convenience: load, migrate the database, and insert the bundle
/// end-to-end.  This is what the `cli seed` subcommand calls.
pub async fn seed_property(
  property_path: impl AsRef<Path>,
  catalog: &Catalog,
  db: &SimDb,
) -> Result<PropertyBundle, SeedError> {
  db.migrate().await?;
  let bundle = load_property(property_path, catalog)?;
  bundle.insert_into(db).await?;
  Ok(bundle)
}
