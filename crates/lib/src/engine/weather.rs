//! Weather model: monthly climatology + seeded stochastic rain.
//!
//! The model has three layers stacked in priority order:
//!
//! 1. Scripted overrides from the scenario (e.g. a July heatwave).
//!    When one covers the target instant, it wins.
//! 2. A seeded stochastic rain generator.  The climatology tells us
//!    how many rain events to expect in a given month; a
//!    deterministic RNG picks when they fire and how hard.
//! 3. Climatology means — temperature, humidity, wind, solar —
//!    supply the baseline always.
//!
//! Same seed + same inputs ⇒ byte-identical output, which is the
//! contract the Phase 3 snapshot tests rely on.

use chrono::{Datelike, NaiveDateTime};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::clock::{SimClock, SimInstant};
use crate::sim::scenario::WeatherOverride;

/// A single weather datum at one instant.  Units are SI throughout;
/// callers that need other units convert at the edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeatherSample {
  pub temperature_c: f64,
  pub humidity_pct: f64,
  pub wind_m_per_s: f64,
  pub solar_w_per_m2: f64,
  /// Instantaneous precipitation rate.  Converted from monthly
  /// totals by the stochastic generator, or forced by an override.
  pub precipitation_mm_per_hour: f64,
}

/// Monthly climatology for one location.  The twelve
/// [`MonthlyNormal`]s are indexed 0..12 for Jan..Dec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Climatology {
  pub name: String,
  pub monthly: [MonthlyNormal; 12],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MonthlyNormal {
  pub mean_temperature_c: f64,
  pub mean_humidity_pct: f64,
  pub mean_wind_m_per_s: f64,
  pub mean_solar_w_per_m2: f64,
  /// Expected total precipitation for the whole month, in mm.  The
  /// stochastic generator distributes this across rain events.
  pub mean_monthly_precip_mm: f64,
  /// Expected number of rain events per month.  A higher number
  /// means more frequent but lighter events for the same total.
  pub rain_events_per_month: u32,
}

impl Climatology {
  /// Portland, OR climatology.  Values are typical residential
  /// approximations; the engine does not claim climate-science
  /// accuracy, only determinism and plausibility.  Month 0 = Jan.
  pub fn portland_or() -> Self {
    Self {
      name: "portland-or".to_string(),
      monthly: [
        // Jan
        MonthlyNormal {
          mean_temperature_c: 5.0,
          mean_humidity_pct: 82.0,
          mean_wind_m_per_s: 3.0,
          mean_solar_w_per_m2: 50.0,
          mean_monthly_precip_mm: 152.0,
          rain_events_per_month: 18,
        },
        // Feb
        MonthlyNormal {
          mean_temperature_c: 6.6,
          mean_humidity_pct: 78.0,
          mean_wind_m_per_s: 3.2,
          mean_solar_w_per_m2: 90.0,
          mean_monthly_precip_mm: 107.0,
          rain_events_per_month: 16,
        },
        // Mar
        MonthlyNormal {
          mean_temperature_c: 9.0,
          mean_humidity_pct: 74.0,
          mean_wind_m_per_s: 3.3,
          mean_solar_w_per_m2: 150.0,
          mean_monthly_precip_mm: 115.0,
          rain_events_per_month: 17,
        },
        // Apr
        MonthlyNormal {
          mean_temperature_c: 11.3,
          mean_humidity_pct: 70.0,
          mean_wind_m_per_s: 3.1,
          mean_solar_w_per_m2: 210.0,
          mean_monthly_precip_mm: 76.0,
          rain_events_per_month: 15,
        },
        // May
        MonthlyNormal {
          mean_temperature_c: 14.6,
          mean_humidity_pct: 68.0,
          mean_wind_m_per_s: 2.9,
          mean_solar_w_per_m2: 260.0,
          mean_monthly_precip_mm: 58.0,
          rain_events_per_month: 12,
        },
        // Jun
        MonthlyNormal {
          mean_temperature_c: 17.9,
          mean_humidity_pct: 66.0,
          mean_wind_m_per_s: 2.8,
          mean_solar_w_per_m2: 290.0,
          mean_monthly_precip_mm: 38.0,
          rain_events_per_month: 8,
        },
        // Jul
        MonthlyNormal {
          mean_temperature_c: 21.1,
          mean_humidity_pct: 62.0,
          mean_wind_m_per_s: 2.7,
          mean_solar_w_per_m2: 320.0,
          mean_monthly_precip_mm: 15.0,
          rain_events_per_month: 3,
        },
        // Aug
        MonthlyNormal {
          mean_temperature_c: 21.6,
          mean_humidity_pct: 62.0,
          mean_wind_m_per_s: 2.6,
          mean_solar_w_per_m2: 290.0,
          mean_monthly_precip_mm: 18.0,
          rain_events_per_month: 3,
        },
        // Sep
        MonthlyNormal {
          mean_temperature_c: 18.3,
          mean_humidity_pct: 66.0,
          mean_wind_m_per_s: 2.5,
          mean_solar_w_per_m2: 230.0,
          mean_monthly_precip_mm: 36.0,
          rain_events_per_month: 7,
        },
        // Oct
        MonthlyNormal {
          mean_temperature_c: 12.8,
          mean_humidity_pct: 76.0,
          mean_wind_m_per_s: 2.7,
          mean_solar_w_per_m2: 140.0,
          mean_monthly_precip_mm: 89.0,
          rain_events_per_month: 14,
        },
        // Nov
        MonthlyNormal {
          mean_temperature_c: 8.0,
          mean_humidity_pct: 82.0,
          mean_wind_m_per_s: 3.0,
          mean_solar_w_per_m2: 60.0,
          mean_monthly_precip_mm: 170.0,
          rain_events_per_month: 18,
        },
        // Dec
        MonthlyNormal {
          mean_temperature_c: 4.7,
          mean_humidity_pct: 84.0,
          mean_wind_m_per_s: 3.0,
          mean_solar_w_per_m2: 40.0,
          mean_monthly_precip_mm: 175.0,
          rain_events_per_month: 19,
        },
      ],
    }
  }

  /// Climatology for the named zone.  Returns `None` when the engine
  /// does not ship one for this name; callers may then fall back to
  /// a user-supplied climatology loaded from their own source.
  pub fn for_zone(name: &str) -> Option<Climatology> {
    match name {
      "portland-or" => Some(Self::portland_or()),
      _ => None,
    }
  }

  fn month_normal(&self, dt: NaiveDateTime) -> &MonthlyNormal {
    let idx = (dt.month0()) as usize;
    &self.monthly[idx]
  }
}

/// Reference daily evapotranspiration for the scenario's climatology,
/// in mm/day.  Driven by month only; the soil module scales by crop
/// coefficient.
pub fn reference_et0_mm_per_day(
  climatology: &Climatology,
  dt: NaiveDateTime,
) -> f64 {
  // Temperature-driven approximation — Hargreaves is the obvious
  // next step but needs extraterrestrial radiation per latitude +
  // day of year.  For v0.1 we use a monotonic function of monthly
  // mean temperature that lines up with published ET0 tables for
  // temperate climates.
  let mean_t = climatology.month_normal(dt).mean_temperature_c;
  // Simple linear fit: 0.5 mm/day at 5 °C, 5.5 mm/day at 22 °C.
  // Clamped to [0.2, 7.0] so off-scale inputs never produce zero or
  // negative ET0.
  ((0.5 + (mean_t - 5.0) * 5.0 / 17.0).max(0.2)).min(7.0)
}

/// Plans every rain event in a given month given a seeded RNG.  The
/// plan is cached so successive [`WeatherModel::sample_at`] calls do
/// not re-generate it (which would be nondeterministic across call
/// patterns).
#[derive(Debug, Clone, Copy)]
struct RainEvent {
  /// Start offset from the beginning of the month, minutes.
  start_minute: i64,
  /// Duration of the event, minutes.
  duration_minutes: i64,
  /// Instantaneous rate during the event, mm/hr.
  rate_mm_per_hour: f64,
}

#[derive(Debug)]
pub struct WeatherModel {
  climatology: Climatology,
  overrides: Vec<WeatherOverride>,
  rng: ChaCha8Rng,
  /// Memoized rain plan per (year, month0) so the same month is not
  /// replanned on repeated sample_at calls.
  rain_plans: HashMap<(i32, u32), Vec<RainEvent>>,
}

impl WeatherModel {
  pub fn new(
    seed: u64,
    climatology: Climatology,
    overrides: Vec<WeatherOverride>,
  ) -> Self {
    Self {
      climatology,
      overrides,
      rng: ChaCha8Rng::seed_from_u64(seed),
      rain_plans: HashMap::new(),
    }
  }

  /// Sample the weather at a simulated instant.  Deterministic given
  /// the seed and the inputs; successive calls for the same instant
  /// return the same sample.
  pub fn sample_at(
    &mut self,
    clock: &SimClock,
    instant: SimInstant,
  ) -> WeatherSample {
    let dt = clock.to_datetime(instant);
    let normal = *self.climatology.month_normal(dt);
    let mut sample = WeatherSample {
      temperature_c: normal.mean_temperature_c,
      humidity_pct: normal.mean_humidity_pct,
      wind_m_per_s: normal.mean_wind_m_per_s,
      solar_w_per_m2: normal.mean_solar_w_per_m2,
      precipitation_mm_per_hour: 0.0,
    };

    // Layer 2: seeded stochastic rain.
    let plan = self
      .rain_plans
      .entry((dt.year(), dt.month0()))
      .or_insert_with(|| plan_month(&mut self.rng, &normal, dt));
    let minute_of_month = minute_of_month(dt);
    for ev in plan {
      if minute_of_month >= ev.start_minute
        && minute_of_month < ev.start_minute + ev.duration_minutes
      {
        sample.precipitation_mm_per_hour = ev.rate_mm_per_hour;
        break;
      }
    }

    // Layer 1: overrides win over both climatology and rain.
    for o in &self.overrides {
      let start = o.offset_minutes;
      let end = start + o.duration_minutes;
      if instant.minutes() >= start && instant.minutes() < end {
        if let Some(t) = o.temperature_c {
          sample.temperature_c = t;
        }
        if let Some(h) = o.humidity_pct {
          sample.humidity_pct = h;
        }
        if let Some(w) = o.wind_m_per_s {
          sample.wind_m_per_s = w;
        }
        if let Some(p) = o.precipitation_mm_per_hour {
          sample.precipitation_mm_per_hour = p;
        }
      }
    }

    sample
  }

  pub fn climatology(&self) -> &Climatology {
    &self.climatology
  }
}

fn minute_of_month(dt: NaiveDateTime) -> i64 {
  let day = dt.day() as i64 - 1;
  let hour = dt.hour() as i64;
  let minute = dt.minute() as i64;
  day * 24 * 60 + hour * 60 + minute
}

fn plan_month(
  rng: &mut ChaCha8Rng,
  normal: &MonthlyNormal,
  dt: NaiveDateTime,
) -> Vec<RainEvent> {
  let days_in_month = days_in_month(dt.year(), dt.month());
  let total_minutes = days_in_month as i64 * 24 * 60;
  if normal.rain_events_per_month == 0 || normal.mean_monthly_precip_mm <= 0.0 {
    return Vec::new();
  }
  let mean_mm_per_event =
    normal.mean_monthly_precip_mm / normal.rain_events_per_month as f64;
  let mut events = Vec::with_capacity(normal.rain_events_per_month as usize);
  for _ in 0..normal.rain_events_per_month {
    // Duration 30..240 minutes.  Rate computed to produce roughly
    // `mean_mm_per_event` of precipitation on average.
    let duration_minutes: i64 = rng.gen_range(30..240);
    let start_minute: i64 =
      rng.gen_range(0..(total_minutes - duration_minutes).max(1));
    // Jitter the magnitude ±50 % so not every event is identical.
    let magnitude_mm = mean_mm_per_event * rng.gen_range(0.5..1.5);
    let rate_mm_per_hour = magnitude_mm / (duration_minutes as f64 / 60.0);
    events.push(RainEvent {
      start_minute,
      duration_minutes,
      rate_mm_per_hour,
    });
  }
  events
}

fn days_in_month(year: i32, month: u32) -> i32 {
  let next_month = if month == 12 {
    chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
  } else {
    chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
  }
  .expect("valid next month");
  let this_month =
    chrono::NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
  (next_month - this_month).num_days() as i32
}

use chrono::Timelike;

#[cfg(test)]
mod tests {
  use super::*;
  use chrono::NaiveDate;

  #[test]
  fn climatology_has_twelve_months() {
    let c = Climatology::portland_or();
    assert_eq!(c.monthly.len(), 12);
    assert_eq!(c.name, "portland-or");
  }

  #[test]
  fn et0_is_monotone_with_month() {
    let c = Climatology::portland_or();
    let jan = reference_et0_mm_per_day(
      &c,
      NaiveDate::from_ymd_opt(2026, 1, 15)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap(),
    );
    let jul = reference_et0_mm_per_day(
      &c,
      NaiveDate::from_ymd_opt(2026, 7, 15)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap(),
    );
    assert!(jul > jan, "summer ET0 ({jul}) must exceed winter ({jan})");
  }

  #[test]
  fn sample_is_deterministic() {
    let c = Climatology::portland_or();
    let clock = SimClock::new(NaiveDate::from_ymd_opt(2026, 7, 15).unwrap());
    let mut a = WeatherModel::new(42, c.clone(), Vec::new());
    let mut b = WeatherModel::new(42, c, Vec::new());
    for m in 0..1000 {
      let inst = SimInstant::from_minutes(m);
      assert_eq!(
        a.sample_at(&clock, inst),
        b.sample_at(&clock, inst),
        "seeded samples diverged at minute {m}"
      );
    }
  }

  #[test]
  fn override_wins_when_active() {
    let c = Climatology::portland_or();
    let clock = SimClock::new(NaiveDate::from_ymd_opt(2026, 7, 15).unwrap());
    let overrides = vec![WeatherOverride {
      offset_minutes: 0,
      duration_minutes: 60,
      temperature_c: Some(40.0),
      humidity_pct: None,
      wind_m_per_s: None,
      precipitation_mm_per_hour: None,
    }];
    let mut m = WeatherModel::new(1, c, overrides);
    let early = m.sample_at(&clock, SimInstant::from_minutes(30));
    let late = m.sample_at(&clock, SimInstant::from_minutes(120));
    assert_eq!(early.temperature_c, 40.0);
    assert!(late.temperature_c < 40.0);
  }
}
