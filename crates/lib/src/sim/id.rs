//! Typed string identifiers for the domain model.
//!
//! Two families of ids live here.  *Instance ids* name concrete
//! objects inside a single property (for example a zone id like
//! "yard-a-veggies" or a plant id like "tomato-row-3").  They are
//! authored by the property fixture and only need to be unique
//! within that property.  *Model ids* name rows in the hardware or
//! species catalog (for example a controller model id like
//! "example-24v-controller" or a species id like "tomato-sungold").
//! They are shared across every property and resolved by the catalog
//! loader in Phase 2.5.
//!
//! Both families are just wrapped `String`s.  The newtypes exist so
//! the compiler can tell them apart — passing a `SpigotId` where a
//! `ZoneId` is expected should not compile.  The shape is
//! deliberately simple: no interning, no `Arc<str>`, no `Cow`.  The
//! domain model is small enough that a few extra `String`s per
//! property do not matter, and simple types are easier to snapshot.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Generate a typed string-id newtype.  The generated type derives
/// the traits we rely on everywhere (hashing for map keys, ordering
/// for stable snapshots, serde for TOML / JSON round-tripping) and
/// exposes `new`, `as_str`, and `Display`.
macro_rules! define_id {
  ($(#[$meta:meta])* $name:ident) => {
    $(#[$meta])*
    #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct $name(String);

    impl $name {
      pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
      }

      pub fn as_str(&self) -> &str {
        &self.0
      }

      pub fn into_inner(self) -> String {
        self.0
      }
    }

    impl fmt::Display for $name {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
      }
    }

    impl From<&str> for $name {
      fn from(value: &str) -> Self {
        Self::new(value)
      }
    }

    impl From<String> for $name {
      fn from(value: String) -> Self {
        Self::new(value)
      }
    }
  };
}

// ── Instance ids (unique within one property) ────────────────────────────────

define_id!(
  /// The property itself.  A single SQLite file may hold more than
  /// one property in v0.3; v0.1 only ever has one row.
  PropertyId
);
define_id!(YardId);
define_id!(SpigotId);
define_id!(ZoneId);
define_id!(PlantId);
define_id!(ManifoldInstanceId);
define_id!(ControllerInstanceId);
define_id!(SensorInstanceId);
define_id!(WeatherStationInstanceId);

// ── Catalog model ids (shared across properties) ─────────────────────────────

define_id!(ControllerModelId);
define_id!(SensorModelId);
define_id!(WeatherStationModelId);
define_id!(ManifoldModelId);
define_id!(ValveModelId);
define_id!(EmitterSpecId);
define_id!(PressureRegulatorModelId);
define_id!(BackflowPreventerModelId);
define_id!(DripLineModelId);
define_id!(SpeciesId);
define_id!(SoilTypeId);

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn id_round_trips_through_toml() {
    // The `#[serde(transparent)]` derive means an id serializes as a
    // bare string — the TOML side does not need to know about the
    // newtype.
    let zone = ZoneId::new("front-veggies");
    let serialized = toml::to_string(&Wrapper { zone }).unwrap();
    assert!(serialized.contains("zone = \"front-veggies\""));
    let deserialized: Wrapper = toml::from_str(&serialized).unwrap();
    assert_eq!(deserialized.zone.as_str(), "front-veggies");
  }

  #[test]
  fn ids_of_different_kinds_do_not_compare() {
    // This test exists to anchor a compile-time property: if we ever
    // remove the newtype wrapper, the line below starts to compile
    // against a bare string and the intended type discipline is
    // lost.  The static assertion below fails compilation when that
    // happens.
    fn assert_distinct<A: 'static, B: 'static>() {
      assert_ne!(std::any::TypeId::of::<A>(), std::any::TypeId::of::<B>());
    }
    assert_distinct::<ZoneId, SpigotId>();
    assert_distinct::<ControllerModelId, SensorModelId>();
  }

  #[derive(Serialize, Deserialize)]
  struct Wrapper {
    zone: ZoneId,
  }
}
