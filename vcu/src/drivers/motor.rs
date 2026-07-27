//! L298N dual H-bridge motor driver: LEDC PWM on EN (1kHz, 10-bit duty),
//! plain GPIO direction control on IN1/IN2.
//!
//! [`MotorChannel`] is one independent H-bridge channel (works for either a
//! channel on one L298N board, or a channel on a second, separate board --
//! S2.5 uses one of each for differential steering). [`Motors`] wraps two
//! channels driven at the same speed, kept from S2 for backward
//! compatibility and as what a future `ServoSteering` will own once wired.

use esp_hal::gpio::{AnyPin, DriveMode, Level, Output, OutputConfig};
use esp_hal::ledc::channel::{self, Channel, ChannelHW, ChannelIFace};
use esp_hal::ledc::timer::{self, Timer, TimerIFace};
use esp_hal::ledc::{Ledc, LowSpeed};
use esp_hal::time::Rate;

use crate::config;

const DUTY_MAX: u32 = (1 << config::MOTOR_PWM_BITS as u32) - 1;

// `timer::config::Duty::Duty10Bit` below is a fixed enum variant, not
// derived from `config::MOTOR_PWM_BITS` -- this assertion makes sure the two
// can't silently drift apart if `MOTOR_PWM_BITS` is ever changed.
const _: () = assert!(config::MOTOR_PWM_BITS == 10);

/// Configures the LEDC timer all four motor channels' EN pins share: 1kHz,
/// `config::MOTOR_PWM_BITS`-bit duty. Call once in `main`, store the result
/// somewhere `'static` (e.g. via `static_cell::StaticCell`), and pass a
/// `&'static` reference to [`MotorChannel::new`].
pub fn configure_timer(ledc: &Ledc<'static>) -> Timer<'static, LowSpeed> {
    let mut timer = ledc.timer::<LowSpeed>(timer::Number::Timer1);
    timer
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty10Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(config::MOTOR_PWM_HZ),
        })
        .expect("motor LEDC timer config is a fixed, valid constant");
    timer
}

/// Maps a raw LEDC channel index (as used by `config::LEDC_CH_*`) to the
/// enum `esp_hal::ledc::channel` actually wants. This project only ever
/// targets the ESP32 classic, which has all eight LEDC channels.
pub fn channel_from_index(n: u8) -> channel::Number {
    match n {
        0 => channel::Number::Channel0,
        1 => channel::Number::Channel1,
        2 => channel::Number::Channel2,
        3 => channel::Number::Channel3,
        4 => channel::Number::Channel4,
        5 => channel::Number::Channel5,
        6 => channel::Number::Channel6,
        7 => channel::Number::Channel7,
        _ => panic!("LEDC channel index out of range"),
    }
}

/// One independent L298N channel: PWM enable pin + two direction pins.
pub struct MotorChannel<'d> {
    enable: Channel<'d, LowSpeed>,
    in1: Output<'d>,
    in2: Output<'d>,
}

impl<'d> MotorChannel<'d> {
    pub fn new(
        timer: &'d Timer<'static, LowSpeed>,
        number: channel::Number,
        en: AnyPin<'d>,
        in1: AnyPin<'d>,
        in2: AnyPin<'d>,
    ) -> Self {
        let mut enable = Channel::new(number, en);
        enable
            .configure(channel::config::Config {
                timer,
                duty_pct: 0,
                drive_mode: DriveMode::PushPull,
            })
            .expect("motor LEDC channel config is a fixed, valid constant");

        Self {
            enable,
            in1: Output::new(in1, Level::Low, OutputConfig::default()),
            in2: Output::new(in2, Level::Low, OutputConfig::default()),
        }
    }

    /// `speed`: -1.0 (reverse) .. 1.0 (forward), 0.0 = coast. Clamped and
    /// NaN-sanitised before application -- belt-and-suspenders on top of
    /// what every caller upstream already guarantees.
    pub fn set_speed(&mut self, speed: f32) {
        let speed = if speed.is_finite() {
            speed.clamp(shared::THROTTLE_MIN, shared::THROTTLE_MAX)
        } else {
            0.0
        };

        let duty = (speed.abs() * DUTY_MAX as f32) as u32;

        // Duty first, direction second: the motor must never spend even one
        // PWM cycle at full speed in the previous direction.
        self.enable.set_duty_hw(duty.min(DUTY_MAX));

        if speed > 0.0 {
            self.in1.set_high();
            self.in2.set_low();
        } else if speed < 0.0 {
            self.in1.set_low();
            self.in2.set_high();
        } else {
            self.in1.set_low();
            self.in2.set_low();
        }
    }

    /// Coasts to a stop: EN pin low, direction pins low.
    pub fn stop(&mut self) {
        self.enable.set_duty_hw(0);
        self.in1.set_low();
        self.in2.set_low();
    }
}

/// Two [`MotorChannel`]s driven at the same speed -- the S2 same-speed API,
/// kept for backward compatibility.
pub struct Motors<'d> {
    left: MotorChannel<'d>,
    right: MotorChannel<'d>,
}

impl<'d> Motors<'d> {
    pub fn new(left: MotorChannel<'d>, right: MotorChannel<'d>) -> Self {
        Self { left, right }
    }

    /// Sets both channels to the same speed. See [`MotorChannel::set_speed`].
    pub fn set_throttle(&mut self, throttle: f32) {
        self.set_speeds(throttle, throttle);
    }

    /// Sets each channel independently.
    pub fn set_speeds(&mut self, left: f32, right: f32) {
        self.left.set_speed(left);
        self.right.set_speed(right);
    }

    /// Coasts both channels to a stop.
    pub fn stop(&mut self) {
        self.left.stop();
        self.right.stop();
    }
}
