//! Reads the two-axis thumbstick at `config::CMD_PUBLISH_HZ` and publishes
//! normalised `DriveCommand`s for `net::transport` to send. Pure input --
//! this node has no knowledge of modes or ESTOP (S5 dashboard-only).

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker};
use esp_hal::Blocking;
use esp_hal::analog::adc::{Adc, AdcChannel, AdcPin};
use esp_hal::peripherals::{ADC1, GPIO34, GPIO35};
use shared::DriveCommand;

use crate::calibration::{Axis, CalibrationData, normalise};
use crate::config;

pub type AxisXPin = AdcPin<GPIO34<'static>, ADC1<'static>>;
pub type AxisYPin = AdcPin<GPIO35<'static>, ADC1<'static>>;
pub type JoystickAdc = Adc<'static, ADC1<'static>, Blocking>;

/// Latest command from the thumbstick. Overwrite (not queue) semantics: only
/// the newest sample matters. Read by `net::transport::transport_task`.
pub static JOYSTICK_CMD: Signal<CriticalSectionRawMutex, DriveCommand> = Signal::new();

/// Reads a single raw ADC count. A transient hardware read error falls back
/// to the calibrated centre (i.e. "no input"), which is always safe: it
/// never becomes a false drive command. Also used by
/// `calibration::calibrate_at_boot` to sample both axes at rest.
pub(crate) fn read_raw<PIN: AdcChannel>(
    adc: &mut JoystickAdc,
    pin: &mut AdcPin<PIN, ADC1<'static>>,
    fallback: u16,
) -> u16 {
    nb::block!(adc.read_oneshot(pin)).unwrap_or(fallback)
}

#[embassy_executor::task]
pub async fn input_task(
    mut adc: JoystickAdc,
    mut axis_x: AxisXPin,
    mut axis_y: AxisYPin,
    cal: CalibrationData,
) -> ! {
    let mut ticker = Ticker::every(Duration::from_hz(config::CMD_PUBLISH_HZ as u64));
    let mut seq: u32 = 0;

    loop {
        let raw_x = read_raw(&mut adc, &mut axis_x, cal.center_x);
        let raw_y = read_raw(&mut adc, &mut axis_y, cal.center_y);

        let steer_norm_raw = normalise(raw_x, &cal, Axis::X);
        let throttle_norm_raw = normalise(raw_y, &cal, Axis::Y);

        let steer_norm = if config::INVERT_X {
            -steer_norm_raw
        } else {
            steer_norm_raw
        };
        let throttle_norm = if config::INVERT_Y {
            -throttle_norm_raw
        } else {
            throttle_norm_raw
        };

        let cmd = DriveCommand {
            steer_deg: steer_norm * shared::STEER_MAX_DEG,
            throttle: throttle_norm,
            seq,
        };
        seq = seq.wrapping_add(1);

        JOYSTICK_CMD.signal(cmd);
        ticker.next().await;
    }
}
