//! Executor-liveness + link-status indicator: the onboard ESP32-CAM red LED
//! (GPIO33, active LOW). Fast blink (4Hz) while WiFi is down, slow blink
//! (1Hz) once connected -- a visual status readable without a serial
//! monitor.

use core::sync::atomic::Ordering;

use embassy_time::{Duration, Timer};
use esp_hal::gpio::Output;

use crate::net::wifi::WIFI_UP;

const FAST_INTERVAL: Duration = Duration::from_millis(125);
const SLOW_INTERVAL: Duration = Duration::from_millis(500);

#[embassy_executor::task]
pub async fn heartbeat_task(mut led: Output<'static>) -> ! {
    loop {
        let fast = !WIFI_UP.load(Ordering::Relaxed);
        let interval = if fast { FAST_INTERVAL } else { SLOW_INTERVAL };

        led.set_low(); // LED on (active LOW)
        Timer::after(interval).await;
        led.set_high(); // LED off
        Timer::after(interval).await;
    }
}
