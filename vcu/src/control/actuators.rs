//! Applies incoming `DriveCommand`s to the rear drive motors and whichever
//! [`crate::steering::Steering`] implementation is active, gated by the
//! arbiter's mode/ESTOP state, and publishes the applied values to
//! [`crate::state::APPLIED_CMD`] for telemetry to read.
//!
//! Since the S2.5 four-motor correction: `rear` (L298N #1) always gets equal
//! throttle on both sides and provides forward/reverse; `steering` (L298N #2)
//! gets the differential-steer math. Both are driven from the same
//! `DriveCommand` every time one is applied.
//!
//! This task is the sole owner of both `rear` and `steering`. The arbiter
//! cannot call their `stop()`/`safe_state()` directly (it doesn't own them),
//! so it raises `state::SAFE_STATE_REQ` instead; this task services that
//! request on every iteration regardless of gating state.

use alloc::boxed::Box;
use core::sync::atomic::Ordering;

use embassy_time::{Duration, Timer};
use shared::{DriveCommand, Mode};

use crate::drivers::motor::Motors;
use crate::state::{self, APPLIED_CMD, RAW_CMD, SAFE_STATE_REQ};
use crate::steering::Steering;

/// How often the actuator task rechecks mode/ESTOP gating while not
/// operable. Short enough that re-arming feels immediate, long enough not
/// to busy-loop.
const GATE_RECHECK: Duration = Duration::from_millis(10);

#[embassy_executor::task]
pub async fn actuator_task(mut rear: Motors<'static>, mut steering: Box<dyn Steering>) -> ! {
    loop {
        if SAFE_STATE_REQ.swap(false, Ordering::SeqCst) {
            rear.stop();
            steering.safe_state();
            *APPLIED_CMD.lock().await = DriveCommand::safe();
            continue;
        }

        let in_manual = matches!(state::active_mode(), Mode::Manual) && !state::estop_latched();
        if !in_manual {
            // Drain (not just skip) any command that arrives while not
            // operable. RAW_CMD is a single-slot Signal, not a queue: if we
            // left a stale value sitting here, the first `wait()` below
            // after re-arming would return it immediately, silently
            // applying a command sent *before* arming instead of requiring
            // a fresh one after. That would violate "a command while ESTOP
            // is latched is silently ignored" (S3 T7) -- ignored has to
            // mean gone, not deferred.
            RAW_CMD.reset();
            Timer::after(GATE_RECHECK).await;
            continue;
        }

        let cmd = RAW_CMD.wait().await;
        // Belt-and-suspenders: shared::DriveCommand::clamped() already ran
        // upstream in net::transport::dispatch_command.
        let cmd = cmd.clamped();

        rear.set_throttle(cmd.throttle);
        steering.apply(cmd.steer_deg, cmd.throttle);

        *APPLIED_CMD.lock().await = cmd;
    }
}
