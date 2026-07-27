//! Heartbeat telemetry: publishes `VcuState` at `shared::STATE_PUBLISH_HZ`,
//! plus an early publish whenever the arbiter signals a mode change so a
//! transition doesn't wait up to 100ms to show up in `vcu/state`.

use core::sync::atomic::Ordering;

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Ticker};
use shared::VcuState;

use crate::net::transport::TELEMETRY;
use crate::state::{self, APPLIED_CMD, LAST_CMD_MS, MODE_CHANGED};

/// Publishes the current heartbeat on a drift-free ticker (or immediately,
/// on a mode change -- see [`MODE_CHANGED`]). `uptime_ms`/`link_ms` are both
/// derived from `state::now_ms()`, which is genuinely monotonic since boot --
/// the acceptance signal that the heartbeat is live rather than a stuck
/// retained message, and that never resets across mode transitions.
#[embassy_executor::task]
pub async fn telemetry_task() -> ! {
    let mut ticker = Ticker::every(Duration::from_hz(shared::STATE_PUBLISH_HZ as u64));
    loop {
        // Race the regular tick against an early wake from the arbiter.
        // Cancel-safe: dropping the losing future (Ticker::next() or
        // Signal::wait()) here loses no tick and no signal (a Signal only
        // remembers "signalled since last successful wait()", so a wake
        // that arrives an instant late is still caught next iteration).
        match select(ticker.next(), MODE_CHANGED.wait()).await {
            Either::First(()) | Either::Second(()) => {}
        }

        let applied = *APPLIED_CMD.lock().await;
        let now = state::now_ms();
        let last_cmd = LAST_CMD_MS.load(Ordering::SeqCst);

        let state = VcuState {
            steer_deg: applied.steer_deg,
            throttle: applied.throttle,
            mode: state::active_mode(),
            estop: state::estop_latched(),
            link_ms: now.saturating_sub(last_cmd),
            seq: applied.seq,
            uptime_ms: now,
        };
        TELEMETRY.signal(state);
    }
}
