//! Executor-liveness indicator, independent of the network: if this stops
//! blinking while the board is powered, the executor is starved or wedged --
//! the fastest diagnostic in every later stage.

use embassy_time::{Duration, Timer};
use esp_hal::gpio::Output;

const BLINK_PERIOD: Duration = Duration::from_millis(500);

#[embassy_executor::task]
pub async fn heartbeat_led_task(mut led: Output<'static>) -> ! {
    loop {
        led.toggle();
        Timer::after(BLINK_PERIOD).await;
    }
}
