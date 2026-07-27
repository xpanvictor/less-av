//! Switchable steering abstraction: differential (active now, S2.5) vs
//! servo-Ackermann (future, once a BEC is wired for the servo). Swapping
//! strategies is a one-line change in `main.rs` -- see the comment there.

pub mod differential;
pub mod servo;

pub use differential::DifferentialSteering;
pub use servo::ServoSteering;

/// The single interface the actuator task uses for all steering
/// implementations. Implementors own whatever hardware they need.
///
/// Not `Send`: `esp_hal::ledc::channel::Channel` holds a `&RegisterBlock`
/// that isn't `Sync`, so anything owning one (like `MotorChannel`, and
/// therefore `DifferentialSteering`) can't be `Send` either. That's fine --
/// `main.rs` spawns tasks with the plain `Spawner`, whose `spawn()` has no
/// `Send` bound (only `SendSpawner`, used for cross-executor handoff and not
/// used anywhere in this project, requires it).
pub trait Steering {
    /// Applies a drive command. Both `steer_deg` and `throttle` arrive
    /// already clamped to their respective ranges and NaN/infinity
    /// sanitised by the caller.
    fn apply(&mut self, steer_deg: f32, throttle: f32);

    /// Immediately brings the vehicle to a safe state: motors stopped,
    /// steering centred. Called on timeout or ESTOP (S3).
    fn safe_state(&mut self);
}
