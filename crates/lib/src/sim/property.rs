//! Top-level property geometry and hydraulic entry points.
//!
//! A `Property` owns an arbitrary number of `Yard`s (not just front
//! and back — side yards, courtyards, and greenhouses are all
//! yards) and `Spigot`s (water entry points with a known mains
//! pressure).  Zones, manifolds, plants, and hardware instances
//! reference these by id; they are defined in sibling modules.
//!
//! The invariant enforced here is structural: ids are unique within
//! the property, dimensions are positive, pressures are plausible.
//! Referential integrity between zones/manifolds/plants/hardware and
//! the property's yards/spigots is enforced one level up, by the
//! loader that assembles an `AssembledProperty` (Phase 5) and calls
//! the cross-entity validators.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::errors::PropertyValidationError;
use super::id::{PropertyId, SpigotId, YardId};

// ── Yard ─────────────────────────────────────────────────────────────────────

/// Raw yard definition as authored in a property TOML.  The only
/// untrusted-input validation that happens here is the conversion to
/// `Yard`; nothing reads `YardRaw` directly outside of
/// `Property::try_from`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YardRaw {
  pub id: YardId,
  pub name: String,
  pub area_sq_ft: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Yard {
  pub id: YardId,
  pub name: String,
  pub area_sq_ft: f64,
}

impl Yard {
  fn try_from_raw(raw: YardRaw) -> Result<Self, PropertyValidationError> {
    if raw.name.trim().is_empty() {
      return Err(PropertyValidationError::BlankYardName(raw.id));
    }
    if !(raw.area_sq_ft > 0.0) {
      return Err(PropertyValidationError::NonPositiveYardArea {
        yard: raw.id,
        area_sq_ft: raw.area_sq_ft,
      });
    }
    Ok(Self {
      id: raw.id,
      name: raw.name,
      area_sq_ft: raw.area_sq_ft,
    })
  }
}

// ── Spigot ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpigotRaw {
  pub id: SpigotId,
  pub mains_pressure_psi: f64,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spigot {
  pub id: SpigotId,
  pub mains_pressure_psi: f64,
  pub notes: Option<String>,
}

impl Spigot {
  fn try_from_raw(raw: SpigotRaw) -> Result<Self, PropertyValidationError> {
    if !(raw.mains_pressure_psi > 0.0) {
      return Err(PropertyValidationError::NonPositiveMainsPressure {
        spigot: raw.id,
        psi: raw.mains_pressure_psi,
      });
    }
    // Residential mains outside [20, 120] psi is almost always a unit
    // mix-up (kPa entered as psi, or bar entered as psi).  The
    // simulator can run with anything positive, but warning early
    // catches the bug at its source.
    if !(20.0..=120.0).contains(&raw.mains_pressure_psi) {
      return Err(PropertyValidationError::ImplausibleMainsPressure {
        spigot: raw.id,
        psi: raw.mains_pressure_psi,
      });
    }
    Ok(Self {
      id: raw.id,
      mains_pressure_psi: raw.mains_pressure_psi,
      notes: raw.notes,
    })
  }
}

// ── Property ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyRaw {
  pub id: PropertyId,
  pub name: String,
  pub lot_area_sq_ft: f64,
  /// Free-form climate-zone label (USDA hardiness, Köppen, or just
  /// "Portland").  The simulator's weather model resolves it via the
  /// catalog in Phase 2.5.
  pub climate_zone: String,
  pub yards: Vec<YardRaw>,
  pub spigots: Vec<SpigotRaw>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
  pub id: PropertyId,
  pub name: String,
  pub lot_area_sq_ft: f64,
  pub climate_zone: String,
  pub yards: Vec<Yard>,
  pub spigots: Vec<Spigot>,
}

impl Property {
  pub fn yard(&self, id: &YardId) -> Option<&Yard> {
    self.yards.iter().find(|y| &y.id == id)
  }

  pub fn spigot(&self, id: &SpigotId) -> Option<&Spigot> {
    self.spigots.iter().find(|s| &s.id == id)
  }
}

impl TryFrom<PropertyRaw> for Property {
  type Error = PropertyValidationError;

  fn try_from(raw: PropertyRaw) -> Result<Self, Self::Error> {
    if raw.name.trim().is_empty() {
      return Err(PropertyValidationError::BlankPropertyName);
    }
    if !(raw.lot_area_sq_ft > 0.0) {
      return Err(PropertyValidationError::NonPositiveLotArea {
        property: raw.id,
        lot_area_sq_ft: raw.lot_area_sq_ft,
      });
    }
    if raw.yards.is_empty() {
      return Err(PropertyValidationError::NoYards(raw.id));
    }
    if raw.spigots.is_empty() {
      return Err(PropertyValidationError::NoSpigots(raw.id));
    }

    let mut seen_yards: HashSet<YardId> = HashSet::new();
    let mut yards: Vec<Yard> = Vec::with_capacity(raw.yards.len());
    for y in raw.yards {
      if !seen_yards.insert(y.id.clone()) {
        return Err(PropertyValidationError::DuplicateYardId(y.id));
      }
      yards.push(Yard::try_from_raw(y)?);
    }

    let mut seen_spigots: HashSet<SpigotId> = HashSet::new();
    let mut spigots: Vec<Spigot> = Vec::with_capacity(raw.spigots.len());
    for s in raw.spigots {
      if !seen_spigots.insert(s.id.clone()) {
        return Err(PropertyValidationError::DuplicateSpigotId(s.id));
      }
      spigots.push(Spigot::try_from_raw(s)?);
    }

    Ok(Property {
      id: raw.id,
      name: raw.name,
      lot_area_sq_ft: raw.lot_area_sq_ft,
      climate_zone: raw.climate_zone,
      yards,
      spigots,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn example_raw() -> PropertyRaw {
    PropertyRaw {
      id: PropertyId::new("example"),
      name: "Example Property".into(),
      lot_area_sq_ft: 6000.0,
      climate_zone: "portland-or".into(),
      yards: vec![
        YardRaw {
          id: YardId::new("yard-a"),
          name: "Yard A".into(),
          area_sq_ft: 2500.0,
        },
        YardRaw {
          id: YardId::new("yard-b"),
          name: "Yard B".into(),
          area_sq_ft: 2500.0,
        },
      ],
      spigots: vec![
        SpigotRaw {
          id: SpigotId::new("spigot-a"),
          mains_pressure_psi: 60.0,
          notes: None,
        },
        SpigotRaw {
          id: SpigotId::new("spigot-b"),
          mains_pressure_psi: 60.0,
          notes: Some("behind the garage".into()),
        },
      ],
    }
  }

  #[test]
  fn happy_path_validates() {
    let prop = Property::try_from(example_raw()).expect("valid property");
    assert_eq!(prop.yards.len(), 2);
    assert_eq!(prop.spigots.len(), 2);
    assert!(prop.yard(&YardId::new("yard-a")).is_some());
    assert!(prop.spigot(&SpigotId::new("spigot-b")).is_some());
  }

  #[test]
  fn blank_name_rejected() {
    let mut raw = example_raw();
    raw.name = "   ".into();
    let err = Property::try_from(raw).unwrap_err();
    assert!(matches!(err, PropertyValidationError::BlankPropertyName));
  }

  #[test]
  fn duplicate_yard_id_rejected() {
    let mut raw = example_raw();
    raw.yards[1].id = raw.yards[0].id.clone();
    let err = Property::try_from(raw).unwrap_err();
    assert!(matches!(err, PropertyValidationError::DuplicateYardId(_)));
  }

  #[test]
  fn non_positive_lot_area_rejected() {
    let mut raw = example_raw();
    raw.lot_area_sq_ft = 0.0;
    let err = Property::try_from(raw).unwrap_err();
    assert!(matches!(err, PropertyValidationError::NonPositiveLotArea { .. }));
  }

  #[test]
  fn implausible_mains_pressure_rejected() {
    let mut raw = example_raw();
    raw.spigots[0].mains_pressure_psi = 500.0; // looks like psi, is actually kPa
    let err = Property::try_from(raw).unwrap_err();
    assert!(matches!(
      err,
      PropertyValidationError::ImplausibleMainsPressure { .. }
    ));
  }

  #[test]
  fn toml_round_trip() {
    let raw = example_raw();
    let text = toml::to_string(&raw).expect("serialize");
    let parsed: PropertyRaw = toml::from_str(&text).expect("parse");
    let prop = Property::try_from(parsed).expect("validate");
    assert_eq!(prop.id.as_str(), "example");
  }
}
