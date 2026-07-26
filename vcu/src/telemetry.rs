//! Heartbeat telemetry: publishes `VcuState` at `shared::STATE_PUBLISH_HZ`.
//!
//! S1 boots ESTOP-latched: the vehicle must require explicit arming before it
//! can move. S3 implements the clear path -- do not "helpfully" boot into
//! MANUAL here.

use embassy_time::{Duration, Instant, Ticker};
use shared::{Mode, VcuState};

use crate::net::transport::TELEMETRY;

/// Publishes the current heartbeat on a drift-free ticker. `boot` is the
/// `Instant` captured at startup, so `uptime_ms` is genuinely monotonic --
/// the acceptance signal that the heartbeat is live rather than a stuck
/// retained message.
#[embassy_executor::task]
pub async fn telemetry_task(boot: Instant) -> ! {
    let mut ticker = Ticker::every(Duration::from_hz(shared::STATE_PUBLISH_HZ as u64));
    loop {
        let state = VcuState {
            steer_deg: 0.0,
            throttle: 0.0,
            mode: Mode::Estop,
            estop: true,
            link_ms: 0,
            seq: 0,
            uptime_ms: Instant::now().duration_since(boot).as_millis() as u32,
        };
        TELEMETRY.signal(state);
        ticker.next().await;
    }
}
