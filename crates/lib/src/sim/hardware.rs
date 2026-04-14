//! Hardware instances: the /specific/ controllers, sensors, and
//! weather stations a property actually owns.
//!
//! The domain model never carries datasheet constants (GPH, minimum
//! inlet pressure, price, max zones).  Those live in the catalog
//! (Phase 2.5) keyed by model id.  An instance here is the placement
//! + wiring + references information only.  That split is what
//! keeps the catalog additive in v0.3: growing the supported-hardware
//! list is TOML data, not code.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::errors::HardwareValidationError;
use super::id::{
  ControllerInstanceId, ControllerModelId, SensorInstanceId, SensorModelId,
  WeatherStationInstanceId, WeatherStationModelId, YardId, ZoneId,
};

// ── ControllerInstance ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerInstanceRaw {
  pub id: ControllerInstanceId,
  pub model_id: ControllerModelId,
  /// Ordered list of zones assigned to output channels 1..N.  The
  /// index into this vector is the physical channel number the
  /// controller's API speaks in.
  pub zone_assignments: Vec<ZoneId>,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControllerInstance {
  pub id: ControllerInstanceId,
  pub model_id: ControllerModelId,
  pub zone_assignments: Vec<ZoneId>,
  pub notes: Option<String>,
}

impl ControllerInstance {
  pub fn try_from_raw(
    raw: ControllerInstanceRaw,
  ) -> Result<Self, HardwareValidationError> {
    let mut seen: HashSet<&ZoneId> = HashSet::new();
    for z in &raw.zone_assignments {
      if !seen.insert(z) {
        return Err(HardwareValidationError::ControllerDoubleAssignedZone(
          z.clone(),
        ));
      }
    }
    Ok(Self {
      id: raw.id,
      model_id: raw.model_id,
      zone_assignments: raw.zone_assignments,
      notes: raw.notes,
    })
  }
}

// ── SensorInstance ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorInstanceRaw {
  pub id: SensorInstanceId,
  pub model_id: SensorModelId,
  /// Which zone this sensor reports for.  One sensor per zone is the
  /// common case but not required; extra sensors outside any zone
  /// are expressed as their own zones in the fixture.
  pub zone_id: ZoneId,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SensorInstance {
  pub id: SensorInstanceId,
  pub model_id: SensorModelId,
  pub zone_id: ZoneId,
  pub notes: Option<String>,
}

impl SensorInstance {
  pub fn try_from_raw(
    raw: SensorInstanceRaw,
  ) -> Result<Self, HardwareValidationError> {
    Ok(Self {
      id: raw.id,
      model_id: raw.model_id,
      zone_id: raw.zone_id,
      notes: raw.notes,
    })
  }
}

// ── WeatherStationInstance ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherStationInstanceRaw {
  pub id: WeatherStationInstanceId,
  pub model_id: WeatherStationModelId,
  /// Where the station is mounted; `None` means "property-level",
  /// i.e. a single station reporting for the whole site.
  #[serde(default)]
  pub yard_id: Option<YardId>,
  #[serde(default)]
  pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherStationInstance {
  pub id: WeatherStationInstanceId,
  pub model_id: WeatherStationModelId,
  pub yard_id: Option<YardId>,
  pub notes: Option<String>,
}

impl WeatherStationInstance {
  pub fn try_from_raw(
    raw: WeatherStationInstanceRaw,
  ) -> Result<Self, HardwareValidationError> {
    Ok(Self {
      id: raw.id,
      model_id: raw.model_id,
      yard_id: raw.yard_id,
      notes: raw.notes,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn controller_raw() -> ControllerInstanceRaw {
    ControllerInstanceRaw {
      id: ControllerInstanceId::new("controller-a"),
      model_id: ControllerModelId::new("example-24v-controller"),
      zone_assignments: vec![
        ZoneId::new("zone-1"),
        ZoneId::new("zone-2"),
        ZoneId::new("zone-3"),
      ],
      notes: None,
    }
  }

  #[test]
  fn controller_happy_path() {
    let c = ControllerInstance::try_from_raw(controller_raw()).expect("valid");
    assert_eq!(c.zone_assignments.len(), 3);
  }

  #[test]
  fn controller_double_assigned_zone_rejected() {
    let mut raw = controller_raw();
    raw.zone_assignments.push(ZoneId::new("zone-1"));
    let err = ControllerInstance::try_from_raw(raw).unwrap_err();
    assert!(matches!(
      err,
      HardwareValidationError::ControllerDoubleAssignedZone(_)
    ));
  }

  #[test]
  fn sensor_happy_path() {
    let s = SensorInstance::try_from_raw(SensorInstanceRaw {
      id: SensorInstanceId::new("sensor-1"),
      model_id: SensorModelId::new("example-soil-sensor"),
      zone_id: ZoneId::new("zone-1"),
      notes: None,
    })
    .expect("valid");
    assert_eq!(s.zone_id.as_str(), "zone-1");
  }

  #[test]
  fn weather_station_property_level() {
    let ws = WeatherStationInstance::try_from_raw(WeatherStationInstanceRaw {
      id: WeatherStationInstanceId::new("ws-1"),
      model_id: WeatherStationModelId::new("example-ws"),
      yard_id: None,
      notes: None,
    })
    .expect("valid");
    assert!(ws.yard_id.is_none());
  }
}
