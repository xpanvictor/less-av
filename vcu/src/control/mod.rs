//! Command dispatch and actuator application.
//!
//! S2 has no safety logic yet: a command arrives and is applied directly.
//! Deadman timeout, mode arbitration, and ESTOP clearing are S3 work.

pub mod actuators;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use shared::DriveCommand;

/// Latest already-clamped command decoded from `cmd/manual`, handed off from
/// [`crate::net::transport`] to [`actuators::actuator_task`]. Overwrite (not
/// queue) semantics: only the newest command matters.
pub static RAW_CMD: Signal<CriticalSectionRawMutex, DriveCommand> = Signal::new();

/// Last command actually applied to the actuators -- the single source of
/// truth [`crate::telemetry`] reads from. Written only by
/// [`actuators::actuator_task`].
pub static APPLIED_CMD: Mutex<CriticalSectionRawMutex, DriveCommand> =
    Mutex::new(DriveCommand::safe());
