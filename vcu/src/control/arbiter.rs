//! Mode arbitration and the ESTOP latch. The single task allowed to write
//! `state::ACTIVE_MODE` and `state::ESTOP_LATCHED`.
//!
//! Runs a ticker at half of `shared::CMD_TIMEOUT_MS` and, on each tick,
//! checks the deadman timeout and drains any pending ESTOP command or mode
//! request. Polling at half the timeout interval bounds worst-case deadman
//! detection to 1.5x the timeout instead of up to 2x -- `Ticker::every`
//! at the full timeout would only check once per window, potentially
//! allowing a silent gap of nearly double the nominal timeout before it's
//! noticed.

use core::sync::atomic::Ordering;

use defmt::{info, warn};
use embassy_time::{Duration, Ticker};
use shared::Mode;

use crate::state::{
    self, ACTIVE_MODE, ESTOP_CMD, ESTOP_LATCHED, MODE_CHANGED, MODE_PUB, MODE_REQ, SAFE_STATE_REQ,
};

const POLL_INTERVAL: Duration = Duration::from_millis((shared::CMD_TIMEOUT_MS / 2) as u64);

#[embassy_executor::task]
pub async fn arbiter_task() -> ! {
    let mut ticker = Ticker::every(POLL_INTERVAL);
    loop {
        ticker.next().await;

        check_deadman();
        handle_estop();
        handle_mode_request();
    }
}

/// Deadman timeout: if no command has arrived on `RAW_CMD` within
/// `CMD_TIMEOUT_MS`, request a safe state. Independent of ESTOP -- a
/// deadman trip does not latch ESTOP or change mode, it only stops the
/// actuators until commands resume (see [`handle_mode_request`] et al. for
/// the T5 recovery path: no re-arm is needed).
fn check_deadman() {
    let now = state::now_ms();
    let last = state::LAST_CMD_MS.load(Ordering::SeqCst);
    let elapsed = now.saturating_sub(last);
    if elapsed > shared::CMD_TIMEOUT_MS {
        SAFE_STATE_REQ.store(true, Ordering::SeqCst);
        warn!("arbiter: command link silent ({}ms), safe state", elapsed);
    }
}

/// Drains a pending ESTOP command, if any. Latching is independent of mode:
/// an assert always wins; a clear is rejected while in Auto mode (S7) but
/// otherwise always allowed. Never panics on anything -- decode failures are
/// handled upstream in `net::transport`, so by the time a command reaches
/// here it's always well-formed.
fn handle_estop() {
    let Some(cmd) = ESTOP_CMD.try_take() else {
        return;
    };

    if cmd.assert {
        ESTOP_LATCHED.store(true, Ordering::SeqCst);
        SAFE_STATE_REQ.store(true, Ordering::SeqCst);
        warn!("ESTOP asserted by {}", cmd.source.as_str());
    } else {
        match state::active_mode() {
            Mode::Auto => warn!("ESTOP clear rejected: in Auto mode"),
            _ => {
                ESTOP_LATCHED.store(false, Ordering::SeqCst);
                info!("ESTOP cleared by {}", cmd.source.as_str());
            }
        }
    }
}

/// Drains a pending mode request, if any, and applies it if the priority
/// rules (`shared::Mode::priority`) allow it: ESTOP is always reachable;
/// MANUAL is blocked while ESTOP is latched; AUTO is not wired until S7 and
/// is always rejected here.
fn handle_mode_request() {
    let Some(req) = MODE_REQ.try_take() else {
        return;
    };

    let estop = state::estop_latched();
    let allowed = match req.mode {
        Mode::Estop => true,
        Mode::Manual => !estop,
        Mode::Auto => {
            warn!("mode request: Auto not yet available (wired in S7)");
            false
        }
    };

    if !allowed {
        return;
    }

    ACTIVE_MODE.store(state::mode_to_u32(&req.mode), Ordering::SeqCst);
    info!("mode -> {}", defmt::Debug2Format(&req.mode));

    if matches!(req.mode, Mode::Estop) {
        // Requesting ESTOP as a *mode* must have the same effect as an
        // ESTOP *assert*: telemetry's `mode` and `estop` fields must not be
        // allowed to diverge (mode=ESTOP with estop=false would be a
        // confusing, unsafe reading), and the vehicle must actually stop --
        // not just stop accepting new commands, which is all a bare mode
        // change achieves on its own (see `control::actuators`).
        ESTOP_LATCHED.store(true, Ordering::SeqCst);
        SAFE_STATE_REQ.store(true, Ordering::SeqCst);
    }

    MODE_PUB.signal(req.mode);
    MODE_CHANGED.signal(());
}
