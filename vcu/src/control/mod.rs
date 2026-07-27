//! Command dispatch, mode/ESTOP arbitration, and actuator application.
//!
//! `arbiter` is the single owner of `state::ACTIVE_MODE` and
//! `state::ESTOP_LATCHED`; `actuators` is the single owner of `steering` and
//! `state::APPLIED_CMD`. See `state.rs` for the statics that connect them.

pub mod actuators;
pub mod arbiter;
