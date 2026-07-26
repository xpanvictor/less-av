//! MG996R servo driver: 50 Hz LEDC PWM, 1000-2000us pulse width maps linearly
//! to [`shared::STEER_MIN_DEG`, `shared::STEER_MAX_DEG`].
//!
//! `Channel::set_duty` only accepts an integer percentage (0-100) of the full
//! 20ms period, which is far too coarse for servo control: our whole +-35
//! degree range spans only 1000us out of the 20000us period, i.e. about 5% of
//! it, so 1%-granularity steps would collapse the whole steering range to a
//! handful of positions. [`esp_hal::ledc::channel::ChannelHW::set_duty_hw`]
//! sets the raw duty register directly, giving the full timer resolution
//! (14-bit here) instead.

use esp_hal::gpio::{AnyPin, DriveMode};
use esp_hal::ledc::channel::{self, Channel, ChannelHW, ChannelIFace};
use esp_hal::ledc::timer::{self, Timer, TimerIFace};
use esp_hal::ledc::{Ledc, LowSpeed};
use esp_hal::time::Rate;

use crate::config;

const PERIOD_US: u32 = 1_000_000 / config::SERVO_PWM_HZ;
const DUTY_BITS: u32 = 14;
const DUTY_MAX: u32 = (1 << DUTY_BITS) - 1;

/// Configures the LEDC timer the servo channel runs on: 50Hz, 14-bit duty.
/// Call once in `main`, store the result somewhere `'static` (e.g. via
/// `static_cell::StaticCell`), and pass a `&'static` reference to
/// [`Servo::new`].
pub fn configure_timer(ledc: &Ledc<'static>) -> Timer<'static, LowSpeed> {
    let mut timer = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    timer
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty14Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(config::SERVO_PWM_HZ),
        })
        .expect("servo LEDC timer config is a fixed, valid constant");
    timer
}

pub struct Servo<'d> {
    channel: Channel<'d, LowSpeed>,
}

impl<'d> Servo<'d> {
    /// Initialises the LEDC channel for the servo pin and centres it.
    pub fn new(timer: &'d Timer<'static, LowSpeed>, pin: AnyPin<'d>) -> Self {
        let mut channel = Channel::new(channel::Number::Channel0, pin);
        channel
            .configure(channel::config::Config {
                timer,
                duty_pct: 0,
                drive_mode: DriveMode::PushPull,
            })
            .expect("servo LEDC channel config is a fixed, valid constant");

        let mut servo = Self { channel };
        servo.center();
        servo
    }

    /// Sets the steering angle in degrees. Clamped to `[STEER_MIN_DEG,
    /// STEER_MAX_DEG]`; NaN/infinity sanitised to 0.0 before the pulse-width
    /// conversion. Never panics on out-of-range input -- `DriveCommand` is
    /// already validated by `shared::DriveCommand::clamped()` upstream, but a
    /// driver that assumes clean input is a liability.
    pub fn set_angle(&mut self, deg: f32) {
        let deg = if deg.is_finite() {
            deg.clamp(shared::STEER_MIN_DEG, shared::STEER_MAX_DEG)
        } else {
            0.0
        };

        let pulse_us = config::SERVO_CENTER_US as f32
            + (deg / shared::STEER_MAX_DEG)
                * (config::SERVO_MAX_US - config::SERVO_CENTER_US) as f32;

        let duty = ((pulse_us * DUTY_MAX as f32) / PERIOD_US as f32) as u32;
        self.channel.set_duty_hw(duty.min(DUTY_MAX));
    }

    /// Returns to centre (0 degrees). Called on the safe state.
    pub fn center(&mut self) {
        self.set_angle(0.0);
    }
}
