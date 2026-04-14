//! Hardware abstraction.
//!
//! Two traits — [`Controller`] and [`SensorSource`] — cover the
//! full read/write surface the server and CLI need.  v0.1 ships
//! only the [`simulated::SimulatedController`] and
//! [`simulated::SimulatedSensorSource`] implementations, both
//! backed by a single [`crate::engine::SimWorld`] so a write
//! through the controller shows up in a read through the sensor
//! source without any bookkeeping at the call site.
//!
//! v0.2 adds concrete driver impls for real controller and sensor
//! hardware; v0.3's planner selects between them based on the
//! runtime `hardware_mode` flag.  Because every consumer talks to
//! the trait, not a specific impl, the planner switch is a config
//! change, not a code change.

pub mod controller;
pub mod errors;
pub mod sensor;
pub mod simulated;

pub use controller::{Controller, ControllerStatus, ZoneStatus};
pub use errors::{ControllerError, SensorError};
pub use sensor::{ReadingKind, SensorReading, SensorSource};
pub use simulated::{SharedWorld, SimulatedController, SimulatedSensorSource};
