//! Irrigation zones and the manifolds that feed them.
//!
//! A `Zone` is a coherent watering unit: a single valve controls it,
//! plants inside it share an irrigation schedule.  Zones live in a
//! yard and are fed by one manifold, which in turn is mounted on one
//! spigot.  A `PlantKind` influences default schedules and emitter
//! recommendations but is deliberately a small, domain-level enum —
//! species detail lives in the catalog.
//!
//! The `Tree` variant is included for properties that *do* irrigate
//! young trees.  At the example property, established trees are not
//! irrigated — that is a property-fixture choice (no zones with
//! `plant-kind = tree`), not a hard-coded rule in this file.

use serde::{Deserialize, Serialize};

use super::errors::ZoneValidationError;
use super::id::{
  EmitterSpecId, ManifoldInstanceId, ManifoldModelId, SoilTypeId, SpigotId,
  YardId, ZoneId,
};

// ── PlantKind ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlantKind {
  VeggieBed,
  Shrub,
  Perennial,
  Tree,
}

impl PlantKind {
  /// Emitter-spec ids the catalog is expected to provide as *typical*
  /// choices for this plant kind.  Used by the soft-warn check; a
  /// zone whose emitter spec does not appear in this list still
  /// validates, but a `tracing::warn` fires.  The catalog, not this
  /// code, is authoritative about which emitters are compatible.
  pub fn typical_emitter_specs(self) -> &'static [&'static str] {
    match self {
      PlantKind::VeggieBed => &["inline-drip-12in", "micro-spray"],
      PlantKind::Shrub => &["1gph-pc", "2gph-pc"],
      PlantKind::Perennial => &["1gph-pc"],
      PlantKind::Tree => &["2gph-pc", "bubbler"],
    }
  }
}

// ── Manifold ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifoldRaw {
  pub id: ManifoldInstanceId,
  pub model_id: ManifoldModelId,
  pub spigot_id: SpigotId,
  /// Maximum number of zones this manifold can serve.  Normally the
  /// catalog provides this via `model_id`, but the fixture may
  /// override it for older or modified hardware.
  pub zone_capacity: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manifold {
  pub id: ManifoldInstanceId,
  pub model_id: ManifoldModelId,
  pub spigot_id: SpigotId,
  pub zone_capacity: i64,
}

impl Manifold {
  pub fn try_from_raw(raw: ManifoldRaw) -> Result<Self, ZoneValidationError> {
    if raw.zone_capacity <= 0 {
      return Err(ZoneValidationError::NonPositiveManifoldCapacity {
        capacity: raw.zone_capacity,
      });
    }
    Ok(Self {
      id: raw.id,
      model_id: raw.model_id,
      spigot_id: raw.spigot_id,
      zone_capacity: raw.zone_capacity,
    })
  }
}

// ── Zone ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneRaw {
  pub id: ZoneId,
  pub yard_id: YardId,
  pub manifold_id: ManifoldInstanceId,
  pub plant_kind: PlantKind,
  pub emitter_spec_id: EmitterSpecId,
  pub soil_type_id: SoilTypeId,
  pub area_sq_ft: f64,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
  pub id: ZoneId,
  pub yard_id: YardId,
  pub manifold_id: ManifoldInstanceId,
  pub plant_kind: PlantKind,
  pub emitter_spec_id: EmitterSpecId,
  pub soil_type_id: SoilTypeId,
  pub area_sq_ft: f64,
  pub notes: Option<String>,
}

impl Zone {
  pub fn try_from_raw(raw: ZoneRaw) -> Result<Self, ZoneValidationError> {
    if !(raw.area_sq_ft > 0.0) {
      return Err(ZoneValidationError::NonPositiveZoneArea {
        zone: raw.id,
        area_sq_ft: raw.area_sq_ft,
      });
    }
    // Soft-warn when the chosen emitter spec is not one of the
    // catalog's typical recommendations for this plant kind.  The
    // caller may still want this pairing (a vegetable bed with
    // point-emitter drip lines is unusual but valid), so this never
    // produces a hard error.
    let typical = raw.plant_kind.typical_emitter_specs();
    if !typical.iter().any(|t| *t == raw.emitter_spec_id.as_str()) {
      tracing::warn!(
        zone = %raw.id,
        plant_kind = ?raw.plant_kind,
        emitter_spec = %raw.emitter_spec_id,
        "zone emitter spec is not a typical pairing for this plant kind; \
         check the fixture if this was unintentional"
      );
    }
    Ok(Self {
      id: raw.id,
      yard_id: raw.yard_id,
      manifold_id: raw.manifold_id,
      plant_kind: raw.plant_kind,
      emitter_spec_id: raw.emitter_spec_id,
      soil_type_id: raw.soil_type_id,
      area_sq_ft: raw.area_sq_ft,
      notes: raw.notes,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn zone_raw() -> ZoneRaw {
    ZoneRaw {
      id: ZoneId::new("yard-a-veggies"),
      yard_id: YardId::new("yard-a"),
      manifold_id: ManifoldInstanceId::new("manifold-a"),
      plant_kind: PlantKind::VeggieBed,
      emitter_spec_id: EmitterSpecId::new("inline-drip-12in"),
      soil_type_id: SoilTypeId::new("silty-clay-loam"),
      area_sq_ft: 120.0,
      notes: None,
    }
  }

  fn manifold_raw() -> ManifoldRaw {
    ManifoldRaw {
      id: ManifoldInstanceId::new("manifold-a"),
      model_id: ManifoldModelId::new("generic-3zone-manifold"),
      spigot_id: SpigotId::new("spigot-a"),
      zone_capacity: 3,
    }
  }

  #[test]
  fn zone_happy_path() {
    let z = Zone::try_from_raw(zone_raw()).expect("valid zone");
    assert_eq!(z.plant_kind, PlantKind::VeggieBed);
  }

  #[test]
  fn zero_area_rejected() {
    let mut raw = zone_raw();
    raw.area_sq_ft = 0.0;
    let err = Zone::try_from_raw(raw).unwrap_err();
    assert!(matches!(err, ZoneValidationError::NonPositiveZoneArea { .. }));
  }

  #[test]
  fn unusual_emitter_still_validates() {
    // Vegetable bed with per-plant 1 GPH emitters is unusual but not
    // an error.  The soft-warn tracing event fires; the constructor
    // returns Ok.
    let mut raw = zone_raw();
    raw.emitter_spec_id = EmitterSpecId::new("1gph-pc");
    Zone::try_from_raw(raw).expect("still valid");
  }

  #[test]
  fn manifold_happy_path() {
    let m = Manifold::try_from_raw(manifold_raw()).expect("valid");
    assert_eq!(m.zone_capacity, 3);
  }

  #[test]
  fn zero_capacity_manifold_rejected() {
    let mut raw = manifold_raw();
    raw.zone_capacity = 0;
    let err = Manifold::try_from_raw(raw).unwrap_err();
    assert!(matches!(
      err,
      ZoneValidationError::NonPositiveManifoldCapacity { .. }
    ));
  }

  #[test]
  fn plant_kind_round_trips_as_kebab_case() {
    let json = serde_json::to_string(&PlantKind::VeggieBed).unwrap();
    assert_eq!(json, "\"veggie-bed\"");
  }
}
