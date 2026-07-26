//! Heartbeat telemetry: publishes `VcuState` at `shared::STATE_PUBLISH_HZ`.
//!
//! S2 has no mode arbitration or deadman timer yet, so `mode`/`estop` are
//! hardcoded to reflect "a command can be applied and nothing is latching
//! it" rather than any real state machine -- S3 wires these to the real
//! thing. Do not build a partial state machine here; just publish honest
//! values for what S2 actually does.

use embassy_time::{Duration, Instant, Ticker};
use shared::{Mode, VcuState};

use crate::control::APPLIED_CMD;
use crate::net::transport::TELEMETRY;

/// Publishes the current heartbeat on a drift-free ticker. `boot` is the
/// `Instant` captured at startup, so `uptime_ms` is genuinely monotonic --
/// the acceptance signal that the heartbeat is live rather than a stuck
/// retained message.
#[embassy_executor::task]
pub async fn telemetry_task(boot: Instant) -> ! {
    let mut ticker = Ticker::every(Duration::from_hz(shared::STATE_PUBLISH_HZ as u64));
    loop {
        let applied = *APPLIED_CMD.lock().await;

        let state = VcuState {
            steer_deg: applied.steer_deg,
            throttle: applied.throttle,
            mode: Mode::Manual,
            estop: false,
            link_ms: 0,
            seq: applied.seq,
            uptime_ms: Instant::now().duration_since(boot).as_millis() as u32,
        };
        TELEMETRY.signal(state);
        ticker.next().await;
    }
}
