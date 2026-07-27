//! Boot-time thumbstick calibration. The centre resting position varies
//! between units; a fixed nominal centre would leave a persistent
//! steer/throttle offset at rest, so this measures the true centre each
//! time the node boots.

use embassy_time::{Duration, Timer};

use crate::config;
use crate::input::{AxisXPin, AxisYPin, JoystickAdc, read_raw};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug)]
pub struct CalibrationData {
    /// Measured ADC count at rest, X axis.
    pub center_x: u16,
    /// Measured ADC count at rest, Y axis.
    pub center_y: u16,
    pub max_x: u16,
    pub max_y: u16,
    pub min_x: u16,
    pub min_y: u16,
}

impl CalibrationData {
    pub fn default_uncalibrated() -> Self {
        Self {
            center_x: config::ADC_CENTER_NOMINAL,
            center_y: config::ADC_CENTER_NOMINAL,
            max_x: config::ADC_MAX,
            max_y: config::ADC_MAX,
            min_x: 0,
            min_y: 0,
        }
    }
}

const CALIBRATION_SAMPLES: u32 = 100;
const CALIBRATION_SAMPLE_INTERVAL: Duration = Duration::from_millis(5);

/// Samples both axes at rest for 100 iterations at 5ms apart (500ms total)
/// and averages them into `center_x`/`center_y`. The operator must not
/// touch the joystick during this window -- see the README boot sequence.
pub async fn calibrate_at_boot(
    adc: &mut JoystickAdc,
    axis_x: &mut AxisXPin,
    axis_y: &mut AxisYPin,
) -> CalibrationData {
    defmt::info!("Calibrating joystick -- release all axes for 500ms...");

    let mut sum_x: u32 = 0;
    let mut sum_y: u32 = 0;
    for _ in 0..CALIBRATION_SAMPLES {
        sum_x += read_raw(adc, axis_x, config::ADC_CENTER_NOMINAL) as u32;
        sum_y += read_raw(adc, axis_y, config::ADC_CENTER_NOMINAL) as u32;
        Timer::after(CALIBRATION_SAMPLE_INTERVAL).await;
    }

    let cal = CalibrationData {
        center_x: (sum_x / CALIBRATION_SAMPLES) as u16,
        center_y: (sum_y / CALIBRATION_SAMPLES) as u16,
        ..CalibrationData::default_uncalibrated()
    };

    defmt::info!(
        "Calibration done: center_x={} center_y={}",
        cal.center_x,
        cal.center_y
    );

    cal
}

/// Converts a raw ADC count to a normalised value in `[-1.0, 1.0]`, `0.0`
/// within the deadzone around the calibrated centre.
pub fn normalise(raw: u16, cal: &CalibrationData, axis: Axis) -> f32 {
    let (center, max, min) = match axis {
        Axis::X => (cal.center_x, cal.max_x, cal.min_x),
        Axis::Y => (cal.center_y, cal.max_y, cal.min_y),
    };

    let delta = (raw as i32) - (center as i32);
    if delta.unsigned_abs() as u16 <= config::DEADZONE_COUNTS {
        return 0.0;
    }

    // Linear map: [min, center) -> [-1, 0), (center, max] -> (0, 1].
    if raw < center {
        let range = (center - min) as f32;
        let offset = (center - raw) as f32;
        -(offset / range).min(1.0)
    } else {
        let range = (max - center) as f32;
        let offset = (raw - center) as f32;
        (offset / range).min(1.0)
    }
}
