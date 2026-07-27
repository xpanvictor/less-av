//! Differential steering: a speed differential between the left and right
//! motors stands in for a physical steering angle. Active strategy until a
//! BEC exists for the servo-Ackermann path (`steering::servo`).
//!
//! `factor = |steer_deg| / STEER_MAX_DEG` ranges from 0.0 (straight) to 1.0
//! (max steer). The inner wheel (the side the vehicle is turning towards)
//! slows to `throttle * (1.0 - factor)`; the outer wheel stays at
//! `throttle`. At max steer the inner wheel stops entirely (tightest turn);
//! at zero steer both sides are equal.
//!
//! Positive `steer_deg` is a right turn (see `shared::DriveCommand`), which
//! makes the **right** wheel the inner one -- not the left. (An earlier
//! draft of this module's spec had the left/right assignment backwards in
//! its prose table; its own worked numeric example for a right turn -- and
//! every worked acceptance test -- labels the right wheel as inner, which is
//! also what a real right turn requires. This implementation follows the
//! worked examples.)

use crate::drivers::motor::MotorChannel;

use super::Steering;

pub struct DifferentialSteering<'d> {
    left: MotorChannel<'d>,
    right: MotorChannel<'d>,
}

impl<'d> DifferentialSteering<'d> {
    pub fn new(left: MotorChannel<'d>, right: MotorChannel<'d>) -> Self {
        Self { left, right }
    }
}

impl<'d> Steering for DifferentialSteering<'d> {
    fn apply(&mut self, steer_deg: f32, throttle: f32) {
        let factor = steer_deg.abs() / shared::STEER_MAX_DEG;
        let inner = throttle * (1.0 - factor);
        let outer = throttle;

        let (left, right) = if steer_deg > 0.0 {
            (outer, inner) // right turn: right wheel is inner (slower)
        } else if steer_deg < 0.0 {
            (inner, outer) // left turn: left wheel is inner (slower)
        } else {
            (throttle, throttle)
        };

        self.left.set_speed(left);
        self.right.set_speed(right);
    }

    fn safe_state(&mut self) {
        self.left.stop();
        self.right.stop();
    }
}
