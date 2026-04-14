//! Plants assigned to zones.
//!
//! A `Plant` is a single planting — one tomato, one rhubarb crown —
//! placed in a specific zone.  Species detail lives in the catalog;
//! the plant carries a `SpeciesId` and the fixture-time overrides
//! that a real garden requires (planting date, a per-plant water
//! need override, free-form notes).  Validation is structural:
//! positive water need inside a plausible residential range.
//! Referential integrity against the zone set is enforced by the
//! property loader.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::errors::PlantValidationError;
use super::id::{PlantId, SpeciesId, ZoneId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantRaw {
  pub id: PlantId,
  pub zone_id: ZoneId,
  pub species_id: SpeciesId,
  pub planted_on: NaiveDate,
  pub water_need_ml_per_day: f64,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plant {
  pub id: PlantId,
  pub zone_id: ZoneId,
  pub species_id: SpeciesId,
  pub planted_on: NaiveDate,
  pub water_need_ml_per_day: f64,
  pub notes: Option<String>,
}

impl Plant {
  pub fn try_from_raw(raw: PlantRaw) -> Result<Self, PlantValidationError> {
    if !(raw.water_need_ml_per_day > 0.0) {
      return Err(PlantValidationError::NonPositiveWaterNeed {
        plant: raw.id,
        water_need_ml_per_day: raw.water_need_ml_per_day,
      });
    }
    // 50 L/day is an extreme upper bound for a single plant in a
    // residential yard.  A value above it almost always means the
    // author put mL/week or gal/day in the field by mistake.
    if raw.water_need_ml_per_day > 50_000.0 {
      return Err(PlantValidationError::ImplausibleWaterNeed {
        plant: raw.id,
        water_need_ml_per_day: raw.water_need_ml_per_day,
      });
    }
    Ok(Self {
      id: raw.id,
      zone_id: raw.zone_id,
      species_id: raw.species_id,
      planted_on: raw.planted_on,
      water_need_ml_per_day: raw.water_need_ml_per_day,
      notes: raw.notes,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn plant_raw() -> PlantRaw {
    PlantRaw {
      id: PlantId::new("tomato-row-3"),
      zone_id: ZoneId::new("yard-a-veggies"),
      species_id: SpeciesId::new("tomato-sungold"),
      planted_on: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
      water_need_ml_per_day: 1500.0,
      notes: Some("staked, south-facing".into()),
    }
  }

  #[test]
  fn plant_happy_path() {
    let p = Plant::try_from_raw(plant_raw()).expect("valid plant");
    assert_eq!(p.species_id.as_str(), "tomato-sungold");
  }

  #[test]
  fn zero_water_need_rejected() {
    let mut raw = plant_raw();
    raw.water_need_ml_per_day = 0.0;
    let err = Plant::try_from_raw(raw).unwrap_err();
    assert!(matches!(err, PlantValidationError::NonPositiveWaterNeed { .. }));
  }

  #[test]
  fn absurd_water_need_rejected() {
    let mut raw = plant_raw();
    // Someone typed "1500 mL/week" → "1500 * 7" expected daily, but
    // then accidentally multiplied again and got 73 500 mL/day.
    raw.water_need_ml_per_day = 73_500.0;
    let err = Plant::try_from_raw(raw).unwrap_err();
    assert!(matches!(err, PlantValidationError::ImplausibleWaterNeed { .. }));
  }
}
