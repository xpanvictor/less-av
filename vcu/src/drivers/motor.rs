//! L298N dual H-bridge motor driver: LEDC PWM on ENA/ENB (1kHz, 10-bit
//! duty), plain GPIO direction control on IN1-IN4.
//!
//! S2 drives both channels identically -- differential drive is S3+ work.

use esp_hal::gpio::{AnyPin, DriveMode, Level, Output, OutputConfig};
use esp_hal::ledc::channel::{self, Channel, ChannelHW, ChannelIFace};
use esp_hal::ledc::timer::{self, Timer, TimerIFace};
use esp_hal::ledc::{Ledc, LowSpeed};
use esp_hal::time::Rate;

use crate::config;

const DUTY_BITS: u32 = 10;
const DUTY_MAX: u32 = (1 << DUTY_BITS) - 1;

/// Configures the LEDC timer both motor channels' EN pins run on: 1kHz,
/// 10-bit duty. Call once in `main`, store the result somewhere `'static`
/// (e.g. via `static_cell::StaticCell`), and pass a `&'static` reference to
/// [`Motors::new`].
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

struct MotorChannel<'d> {
    enable: Channel<'d, LowSpeed>,
    in_a: Output<'d>,
    in_b: Output<'d>,
}

impl<'d> MotorChannel<'d> {
    fn new(
        timer: &'d Timer<'static, LowSpeed>,
        number: channel::Number,
        en: AnyPin<'d>,
        in_a: AnyPin<'d>,
        in_b: AnyPin<'d>,
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
            in_a: Output::new(in_a, Level::Low, OutputConfig::default()),
            in_b: Output::new(in_b, Level::Low, OutputConfig::default()),
        }
    }

    /// `throttle` must already be clamped to `[-1.0, 1.0]` and NaN-sanitised.
    fn set_throttle(&mut self, throttle: f32) {
        let duty = (throttle.abs() * DUTY_MAX as f32) as u32;

        // Duty first, direction second: the motor must never spend even one
        // PWM cycle at full speed in the previous direction.
        self.enable.set_duty_hw(duty.min(DUTY_MAX));

        if throttle > 0.0 {
            self.in_a.set_high();
            self.in_b.set_low();
        } else if throttle < 0.0 {
            self.in_a.set_low();
            self.in_b.set_high();
        } else {
            self.in_a.set_low();
            self.in_b.set_low();
        }
    }

    fn stop(&mut self) {
        self.enable.set_duty_hw(0);
        self.in_a.set_low();
        self.in_b.set_low();
    }
}

pub struct Motors<'d> {
    a: MotorChannel<'d>,
    b: MotorChannel<'d>,
}

impl<'d> Motors<'d> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        timer: &'d Timer<'static, LowSpeed>,
        ena: AnyPin<'d>,
        in1: AnyPin<'d>,
        in2: AnyPin<'d>,
        enb: AnyPin<'d>,
        in3: AnyPin<'d>,
        in4: AnyPin<'d>,
    ) -> Self {
        Self {
            a: MotorChannel::new(timer, channel::Number::Channel1, ena, in1, in2),
            b: MotorChannel::new(timer, channel::Number::Channel2, enb, in3, in4),
        }
    }

    /// Sets both channels to the same speed. `throttle`: -1.0 (full reverse)
    /// .. 1.0 (full forward), 0.0 = stop. Clamped and NaN-sanitised before
    /// application -- belt-and-suspenders on top of
    /// `shared::DriveCommand::clamped()` upstream.
    pub fn set_throttle(&mut self, throttle: f32) {
        let throttle = if throttle.is_finite() {
            throttle.clamp(shared::THROTTLE_MIN, shared::THROTTLE_MAX)
        } else {
            0.0
        };
        self.a.set_throttle(throttle);
        self.b.set_throttle(throttle);
    }

    /// Coasts to a stop: EN pins low, direction pins low.
    pub fn stop(&mut self) {
        self.a.stop();
        self.b.stop();
    }
}
