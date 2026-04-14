//! Simulated clock.
//!
//! Everywhere in the engine that would otherwise call
//! `chrono::Utc::now()` takes a [`SimInstant`] instead.  This keeps
//! simulation deterministic (Phase 3 needs byte-identical snapshot
//! output) and lets scenarios run faster or slower than wall clock.
//!
//! [`SimInstant`] is a plain offset (in seconds) from the scenario's
//! start date.  Converting back to a [`NaiveDateTime`] goes through
//! [`SimClock::to_datetime`] so only one place in the engine knows
//! the start date.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

/// A point in simulated time, measured in seconds after the
/// scenario start.  Monotonic and `Copy` so it is cheap to pass
/// around and store in maps.
#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  PartialOrd,
  Ord,
  Hash,
  Serialize,
  Deserialize,
)]
pub struct SimInstant {
  secs_since_start: i64,
}

impl SimInstant {
  pub const START: SimInstant = SimInstant {
    secs_since_start: 0,
  };

  pub fn from_minutes(minutes: i64) -> Self {
    Self {
      secs_since_start: minutes.saturating_mul(60),
    }
  }

  pub fn from_seconds(secs: i64) -> Self {
    Self {
      secs_since_start: secs,
    }
  }

  pub fn seconds(self) -> i64 {
    self.secs_since_start
  }

  pub fn minutes(self) -> i64 {
    self.secs_since_start / 60
  }
}

/// A span of simulated time.  Produced by the `minutes` and `seconds`
/// constructors, consumed by [`SimClock::step`] and anywhere a
/// "duration" appears in a simulated schedule.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct SimDuration {
  secs: i64,
}

impl SimDuration {
  pub fn minutes(minutes: i64) -> Self {
    Self {
      secs: minutes.saturating_mul(60),
    }
  }

  pub fn seconds(secs: i64) -> Self {
    Self { secs }
  }

  pub fn hours(hours: i64) -> Self {
    Self::minutes(hours.saturating_mul(60))
  }

  pub fn days(days: i64) -> Self {
    Self::hours(days.saturating_mul(24))
  }

  pub fn total_seconds(self) -> i64 {
    self.secs
  }

  pub fn total_minutes(self) -> i64 {
    self.secs / 60
  }
}

/// The scenario clock.  Owns the start date once at construction;
/// callers ask for the current instant or advance time in discrete
/// steps.  The clock never moves backward.
#[derive(Debug, Clone)]
pub struct SimClock {
  start_date: NaiveDate,
  /// Time-of-day offset within the start date, in seconds.  Defaults
  /// to midnight when constructed via [`SimClock::new`].
  start_time_secs: i64,
  now: SimInstant,
}

impl SimClock {
  pub fn new(start_date: NaiveDate) -> Self {
    Self {
      start_date,
      start_time_secs: 0,
      now: SimInstant::START,
    }
  }

  pub fn with_start_time(start_date: NaiveDate, start_time: NaiveTime) -> Self {
    let start_time_secs = start_time
      .signed_duration_since(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
      .num_seconds();
    Self {
      start_date,
      start_time_secs,
      now: SimInstant::START,
    }
  }

  pub fn now(&self) -> SimInstant {
    self.now
  }

  pub fn start_date(&self) -> NaiveDate {
    self.start_date
  }

  /// Advance the clock by `duration`.  The engine's top-level
  /// [`crate::engine::world::SimWorld::advance`] steps in
  /// one-minute increments so the ODE integrator sees uniform
  /// sub-steps; callers that want coarser steps should still route
  /// through `SimWorld::advance` and let it break the span down.
  pub fn step(&mut self, duration: SimDuration) {
    self.now = SimInstant {
      secs_since_start: self.now.secs_since_start.saturating_add(duration.secs),
    };
  }

  /// Convert a simulated instant into a wall-clock `NaiveDateTime`,
  /// using the scenario's start date + start-of-day offset as the
  /// epoch.  The simulation never reads real wall-clock time, so
  /// every user-facing timestamp flows through here.
  pub fn to_datetime(&self, instant: SimInstant) -> NaiveDateTime {
    let total_secs = self
      .start_time_secs
      .saturating_add(instant.secs_since_start);
    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let date = self.start_date + chrono::Duration::days(days);
    let time =
      NaiveTime::from_num_seconds_from_midnight_opt(secs_of_day as u32, 0)
        .expect("secs_of_day is in [0, 86_400)");
    date.and_time(time)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn step_advances_now_monotonically() {
    let mut c = SimClock::new(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    assert_eq!(c.now().seconds(), 0);
    c.step(SimDuration::minutes(60));
    assert_eq!(c.now().minutes(), 60);
    c.step(SimDuration::minutes(5));
    assert_eq!(c.now().minutes(), 65);
  }

  #[test]
  fn to_datetime_crosses_midnight() {
    let c = SimClock::with_start_time(
      NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
      NaiveTime::from_hms_opt(23, 30, 0).unwrap(),
    );
    let dt = c.to_datetime(SimInstant::from_minutes(45));
    // 23:30 + 45 min = 00:15 the next day.
    assert_eq!(dt.date(), NaiveDate::from_ymd_opt(2026, 7, 2).unwrap());
    assert_eq!(dt.time(), NaiveTime::from_hms_opt(0, 15, 0).unwrap());
  }

  #[test]
  fn duration_accessors_agree() {
    let d = SimDuration::hours(2);
    assert_eq!(d.total_minutes(), 120);
    assert_eq!(d.total_seconds(), 7200);
  }
}
