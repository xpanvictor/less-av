//! Cross-task shared state. Statics and their types only -- no business
//! logic. See `control::arbiter` for the state machine that owns
//! `ACTIVE_MODE` and `ESTOP_LATCHED`, and `control::actuators` for the only
//! other task allowed to touch `SAFE_STATE_REQ`/`APPLIED_CMD`.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Instant;
use shared::{DriveCommand, EstopCommand, Mode, ModeRequest};

// --- Inbound ---------------------------------------------------------------

/// Raw command from transport, before arbitration. Signal: latest wins.
pub static RAW_CMD: Signal<CriticalSectionRawMutex, DriveCommand> = Signal::new();

/// Milliseconds since boot (see [`now_ms`]) at the last message received on
/// `RAW_CMD`. Written by the transport task, read by the arbiter for the
/// deadman check.
///
/// `u32`, not `u64`: Xtensa (this target) has no native 64-bit atomics, so
/// `core::sync::atomic::AtomicU64` doesn't exist here. `u32` milliseconds
/// wraps after ~49.7 days of continuous uptime, matching `VcuState::uptime_ms`
/// (also `u32`), which is more than sufficient for a bench-rig demo.
pub static LAST_CMD_MS: AtomicU32 = AtomicU32::new(0);

/// Requested mode changes (`ModeRequest` from the broker).
pub static MODE_REQ: Signal<CriticalSectionRawMutex, ModeRequest> = Signal::new();

/// ESTOP assertion/clear commands (`EstopCommand` from the broker).
pub static ESTOP_CMD: Signal<CriticalSectionRawMutex, EstopCommand> = Signal::new();

// --- Runtime flags -----------------------------------------------------------

/// Latched ESTOP state. `true` = ESTOP active. Written only by the arbiter.
pub static ESTOP_LATCHED: AtomicBool = AtomicBool::new(true); // boot = safe

/// Current active mode, encoded via [`mode_to_u32`]/[`u32_to_mode`]. Written
/// only by the arbiter. Use `Ordering::SeqCst` for all reads and writes --
/// these are safety-critical flags read across tasks.
pub static ACTIVE_MODE: AtomicU32 = AtomicU32::new(0); // boot = Estop

/// Set by the arbiter when the actuators must immediately go to a safe
/// state (deadman timeout or ESTOP assert). The actuator task is the sole
/// owner of `steering`, so the arbiter cannot call `safe_state()` itself --
/// it raises this flag instead and the actuator task services it on its
/// next iteration. This is the only coupling between the two tasks.
pub static SAFE_STATE_REQ: AtomicBool = AtomicBool::new(false);

/// Signalled by the arbiter on a successful mode transition so telemetry can
/// wake and publish the new `vcu/state` immediately instead of waiting for
/// the next 100Hz tick.
///
/// Deviation from spec: the handoff spec describes this as a plain
/// `AtomicBool` that telemetry "checks on each tick." But telemetry already
/// publishes unconditionally on every tick (10Hz) regardless of any flag, so
/// a flag that's only ever read at an already-scheduled tick boundary can't
/// actually reduce latency below the existing ~100ms cadence -- it would be
/// purely decorative. A `Signal` that telemetry races against its `Ticker`
/// via `select` gives a real early wake (see `telemetry::telemetry_task`),
/// which is what "publish immediately" actually requires.
pub static MODE_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Signalled by the arbiter on a successful mode transition. `transport_task`
/// selects on this to publish the new mode to `TOPIC_MODE_CURRENT`, retained.
pub static MODE_PUB: Signal<CriticalSectionRawMutex, Mode> = Signal::new();

// --- Outbound ----------------------------------------------------------------

/// Last command actually applied to the actuators -- the single source of
/// truth telemetry reads from. Written only by the actuator task.
pub static APPLIED_CMD: Mutex<CriticalSectionRawMutex, DriveCommand> =
    Mutex::new(DriveCommand::safe());

/// Milliseconds since system startup. `Instant` is already ticks-since-boot
/// (see `embassy_time::Instant`'s docs), so this needs no separately
/// threaded-through boot reference. `u32`, matching `LAST_CMD_MS` and
/// `VcuState::uptime_ms`.
pub fn now_ms() -> u32 {
    Instant::now().duration_since(Instant::MIN).as_millis() as u32
}

pub fn mode_to_u32(m: &Mode) -> u32 {
    match m {
        Mode::Estop => 0,
        Mode::Manual => 1,
        Mode::Auto => 2,
    }
}

pub fn u32_to_mode(v: u32) -> Mode {
    match v {
        1 => Mode::Manual,
        2 => Mode::Auto,
        _ => Mode::Estop,
    }
}

pub fn active_mode() -> Mode {
    u32_to_mode(ACTIVE_MODE.load(Ordering::SeqCst))
}

pub fn estop_latched() -> bool {
    ESTOP_LATCHED.load(Ordering::SeqCst)
}
