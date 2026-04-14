//! Scenario descriptions — deterministic, reproducible inputs to the
//! simulation engine.
//!
//! A `Scenario` names a start date, duration, and seed.  Weather
//! overrides displace the climatology model (e.g. a scripted
//! heatwave); manual interventions let the author simulate the user
//! hitting the "run zone now" button at a specific simulated time.
//! The engine (Phase 3) replays these in order to produce the
//! soil-moisture time series.  Because every input is serializable
//! and the seed is explicit, two runs produce byte-identical output.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::errors::ScenarioValidationError;
use super::id::ZoneId;

// ── WeatherOverride ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeatherOverride {
  /// Offset from scenario start, in simulated minutes.
  pub offset_minutes: i64,
  /// Wall-clock span of this override, again in simulated minutes.
  pub duration_minutes: i64,
  /// Optional forced temperature for the span.  When `None`, the
  /// climatology value stands.
  #[serde(default)]
  pub temperature_c: Option<f64>,
  #[serde(default)]
  pub humidity_pct: Option<f64>,
  #[serde(default)]
  pub wind_m_per_s: Option<f64>,
  #[serde(default)]
  pub precipitation_mm_per_hour: Option<f64>,
}

// ── ManualIntervention ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ManualIntervention {
  /// The user hit "run this zone for N minutes" through the UI.
  RunZone {
    offset_minutes: i64,
    zone_id: ZoneId,
    duration_minutes: i64,
  },
  /// The user hit "stop this zone right now".  No-op if the zone is
  /// not already open at the target offset.
  StopZone {
    offset_minutes: i64,
    zone_id: ZoneId,
  },
}

impl ManualIntervention {
  fn offset_minutes(&self) -> i64 {
    match self {
      Self::RunZone { offset_minutes, .. }
      | Self::StopZone { offset_minutes, .. } => *offset_minutes,
    }
  }

  fn zone_id(&self) -> &ZoneId {
    match self {
      Self::RunZone { zone_id, .. } | Self::StopZone { zone_id, .. } => zone_id,
    }
  }
}

// ── Scenario ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRaw {
  pub name: String,
  pub start_date: NaiveDate,
  pub duration_minutes: i64,
  /// Seed for the stochastic rain generator.  Same seed ⇒ identical
  /// output across runs and platforms.
  pub rng_seed: u64,
  #[serde(default)]
  pub weather_overrides: Vec<WeatherOverride>,
  #[serde(default)]
  pub manual_interventions: Vec<ManualIntervention>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
  pub name: String,
  pub start_date: NaiveDate,
  pub duration_minutes: i64,
  pub rng_seed: u64,
  pub weather_overrides: Vec<WeatherOverride>,
  pub manual_interventions: Vec<ManualIntervention>,
}

impl Scenario {
  /// Validate a scenario in isolation.  Cross-checking against a
  /// specific property's zones happens in [`Scenario::validate_against`]
  /// below; the `try_from_raw` path deliberately does not take a
  /// zone set so scenarios can be authored and unit-tested without
  /// a property fixture.
  pub fn try_from_raw(
    raw: ScenarioRaw,
  ) -> Result<Self, ScenarioValidationError> {
    if raw.name.trim().is_empty() {
      return Err(ScenarioValidationError::BlankScenarioName);
    }
    if raw.duration_minutes <= 0 {
      return Err(ScenarioValidationError::NonPositiveDuration {
        duration_minutes: raw.duration_minutes,
      });
    }
    for o in &raw.weather_overrides {
      if o.offset_minutes + o.duration_minutes > raw.duration_minutes {
        return Err(ScenarioValidationError::WeatherOverrideBeyondDuration {
          offset_minutes: o.offset_minutes,
          duration_minutes: raw.duration_minutes,
        });
      }
    }
    for m in &raw.manual_interventions {
      if m.offset_minutes() > raw.duration_minutes {
        return Err(ScenarioValidationError::InterventionBeyondDuration {
          offset_minutes: m.offset_minutes(),
          duration_minutes: raw.duration_minutes,
        });
      }
    }
    Ok(Self {
      name: raw.name,
      start_date: raw.start_date,
      duration_minutes: raw.duration_minutes,
      rng_seed: raw.rng_seed,
      weather_overrides: raw.weather_overrides,
      manual_interventions: raw.manual_interventions,
    })
  }

  /// Cross-validate a scenario against the property's zone set.
  /// Called by the loader after both are validated in isolation.
  pub fn validate_against<'a, I>(
    &self,
    known_zones: I,
  ) -> Result<(), ScenarioValidationError>
  where
    I: IntoIterator<Item = &'a ZoneId>,
  {
    let known: std::collections::HashSet<&ZoneId> =
      known_zones.into_iter().collect();
    for m in &self.manual_interventions {
      if !known.contains(m.zone_id()) {
        return Err(ScenarioValidationError::InterventionUnknownZone(
          m.zone_id().clone(),
        ));
      }
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn scenario_raw() -> ScenarioRaw {
    ScenarioRaw {
      name: "july-heatwave".into(),
      start_date: NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
      duration_minutes: 60 * 24 * 7, // one week
      rng_seed: 42,
      weather_overrides: vec![WeatherOverride {
        offset_minutes: 0,
        duration_minutes: 60 * 24 * 3,
        temperature_c: Some(38.0),
        humidity_pct: Some(20.0),
        wind_m_per_s: None,
        precipitation_mm_per_hour: None,
      }],
      manual_interventions: vec![ManualIntervention::RunZone {
        offset_minutes: 60,
        zone_id: ZoneId::new("zone-1"),
        duration_minutes: 15,
      }],
    }
  }

  #[test]
  fn happy_path() {
    let s = Scenario::try_from_raw(scenario_raw()).expect("valid");
    assert_eq!(s.rng_seed, 42);
  }

  #[test]
  fn blank_name_rejected() {
    let mut raw = scenario_raw();
    raw.name = "  ".into();
    let err = Scenario::try_from_raw(raw).unwrap_err();
    assert!(matches!(err, ScenarioValidationError::BlankScenarioName));
  }

  #[test]
  fn weather_override_past_end_rejected() {
    let mut raw = scenario_raw();
    raw.weather_overrides[0].offset_minutes = raw.duration_minutes - 10;
    raw.weather_overrides[0].duration_minutes = 60 * 24;
    let err = Scenario::try_from_raw(raw).unwrap_err();
    assert!(matches!(
      err,
      ScenarioValidationError::WeatherOverrideBeyondDuration { .. }
    ));
  }

  #[test]
  fn unknown_zone_caught_by_cross_check() {
    let s = Scenario::try_from_raw(scenario_raw()).unwrap();
    let known = vec![ZoneId::new("zone-2")]; // "zone-1" absent
    let err = s.validate_against(known.iter()).unwrap_err();
    assert!(matches!(err, ScenarioValidationError::InterventionUnknownZone(_)));
  }

  #[test]
  fn cross_check_ok_when_zone_present() {
    let s = Scenario::try_from_raw(scenario_raw()).unwrap();
    let known = vec![ZoneId::new("zone-1")];
    s.validate_against(known.iter()).expect("ok");
  }
}
