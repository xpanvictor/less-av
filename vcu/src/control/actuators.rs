//! Applies incoming `DriveCommand`s to the servo and motors, and publishes
//! the applied values to [`super::APPLIED_CMD`] for telemetry to read.

use crate::drivers::motor::Motors;
use crate::drivers::servo::Servo;

use super::{APPLIED_CMD, RAW_CMD};

#[embassy_executor::task]
pub async fn actuator_task(mut servo: Servo<'static>, mut motors: Motors<'static>) -> ! {
    loop {
        let cmd = RAW_CMD.wait().await;

        servo.set_angle(cmd.steer_deg);
        motors.set_throttle(cmd.throttle);

        *APPLIED_CMD.lock().await = cmd;
    }
}
