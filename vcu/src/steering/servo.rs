//! Servo-Ackermann steering -- compile-only stub.
//!
//! Exists so the code compiles and the one-line switch in `main.rs` is
//! demonstrably real. Does nothing at runtime: no BEC is wired for the
//! servo yet, so there is no hardware for this to own.

use super::Steering;

#[allow(dead_code)]
pub struct ServoSteering; // no fields -- servo driver not initialised without BEC

impl ServoSteering {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ServoSteering {
    fn default() -> Self {
        Self::new()
    }
}

impl Steering for ServoSteering {
    fn apply(&mut self, _steer_deg: f32, _throttle: f32) {
        // TODO S2.5->servo: initialise LEDC + BEC, wire drivers::servo::Servo
        // and drivers::motor::Motors here. When ready:
        //   servo.set_angle(steer_deg); motors.set_throttle(throttle);
    }

    fn safe_state(&mut self) {
        // no-op until wired
    }
}
