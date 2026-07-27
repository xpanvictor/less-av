//! MG996R servo driver -- stub.
//!
//! The servo needs a dedicated 5-6V BEC (>=3A); see docs/HARDWARE.md's power
//! warning. That BEC is not wired to the bench rig yet, so this stub keeps
//! the driver's public shape stable without touching LEDC at all, meaning
//! nothing can accidentally drive the servo pin before the BEC exists. The
//! real LEDC implementation (50Hz timer, 14-bit duty, `set_duty_hw` pulse
//! math) lived here during S2 and is recoverable from git history once the
//! BEC is available.

#[allow(dead_code)]
pub struct Servo;

impl Servo {
    pub fn new() -> Self {
        Self
    }

    /// No-op until the BEC is wired and the real LEDC driver is restored.
    pub fn set_angle(&mut self, _deg: f32) {}

    /// No-op until the BEC is wired and the real LEDC driver is restored.
    pub fn center(&mut self) {
        self.set_angle(0.0);
    }
}

impl Default for Servo {
    fn default() -> Self {
        Self::new()
    }
}
