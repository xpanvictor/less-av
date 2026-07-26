//! LESS shared protocol contract.
//!
//! Single source of truth for MQTT topic names and message wire formats
//! shared by every node (VCU, joystick, dashboard, camera, autonomy). No
//! hardware dependencies: compiles `no_std` for firmware and `std` for host
//! tooling/tests.
#![cfg_attr(not(test), no_std)]

use serde::{Deserialize, Serialize};

/// Bump this and the topic root below together when the wire contract changes
/// incompatibly.
pub const PROTOCOL_VERSION: u32 = 1;

macro_rules! topic_root {
    () => {
        "less/v1"
    };
}

pub const TOPIC_ROOT: &str = topic_root!();

pub const TOPIC_CMD_MANUAL: &str = concat!(topic_root!(), "/cmd/manual");
pub const TOPIC_CMD_AUTO: &str = concat!(topic_root!(), "/cmd/auto");
pub const TOPIC_MODE_REQUEST: &str = concat!(topic_root!(), "/mode/request");
pub const TOPIC_MODE_CURRENT: &str = concat!(topic_root!(), "/mode/current");
pub const TOPIC_ESTOP: &str = concat!(topic_root!(), "/estop");
pub const TOPIC_VCU_STATE: &str = concat!(topic_root!(), "/vcu/state");
pub const TOPIC_VCU_STATUS: &str = concat!(topic_root!(), "/vcu/status");

/// Debugging/subscribe-all wildcard. Not a publish target.
pub const TOPIC_ALL: &str = concat!(topic_root!(), "/#");

// --- Limits -----------------------------------------------------------
//
// Bench rig has no chassis, so steering is clamped symmetrically. The real
// vehicle's Ackermann geometry gives asymmetric limits (inner 40 deg, outer
// 26.1 deg); replace this pair once a chassis exists. See docs/PROTOCOL.md.
pub const STEER_MAX_DEG: f32 = 35.0;
pub const STEER_MIN_DEG: f32 = -35.0;
pub const THROTTLE_MAX: f32 = 1.0;
pub const THROTTLE_MIN: f32 = -1.0;

/// Deadman: no accepted command within this window forces the safe state.
pub const CMD_TIMEOUT_MS: u32 = 300;
pub const STATE_PUBLISH_HZ: u32 = 10;
pub const CMD_PUBLISH_HZ: u32 = 50;
pub const MAX_PAYLOAD_CMD: usize = 128;
pub const MAX_PAYLOAD_STATE: usize = 256;

// --- Message types ------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct DriveCommand {
    #[serde(rename = "steer")]
    pub steer_deg: f32,
    pub throttle: f32,
    pub seq: u32,
}

impl DriveCommand {
    /// Clamps into the legal range, replacing NaN/infinity with 0.0. NaN
    /// reaching the servo math produces undefined PWM, so this is mandatory
    /// on every path that accepts an externally-sourced command.
    pub fn clamped(self) -> Self {
        let steer_deg = if self.steer_deg.is_finite() {
            self.steer_deg.clamp(STEER_MIN_DEG, STEER_MAX_DEG)
        } else {
            0.0
        };
        let throttle = if self.throttle.is_finite() {
            self.throttle.clamp(THROTTLE_MIN, THROTTLE_MAX)
        } else {
            0.0
        };
        Self {
            steer_deg,
            throttle,
            seq: self.seq,
        }
    }

    /// Canonical safe state used by every timeout/fault path.
    pub const fn safe() -> Self {
        Self {
            steer_deg: 0.0,
            throttle: 0.0,
            seq: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Mode {
    Estop,
    Manual,
    Auto,
}

impl Mode {
    /// Higher wins. S3 arbitration relies on this ordering.
    pub fn priority(&self) -> u8 {
        match self {
            Mode::Estop => 3,
            Mode::Manual => 2,
            Mode::Auto => 1,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ModeRequest {
    pub mode: Mode,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct EstopCommand {
    /// `true` = engage ESTOP, `false` = request clear.
    pub assert: bool,
    /// Who asserted it, for telemetry.
    #[serde(rename = "src")]
    pub source: EstopSource,
}

/// Fixed-capacity, `Copy` string (max 16 bytes) for [`EstopCommand::source`].
///
/// `heapless::String` was the spec's first choice but its backing `Vec` does
/// not implement `Copy`, which every message type in this crate must. This
/// wraps a plain byte buffer instead while still (de)serializing as a JSON
/// string, matching the spec's documented fallback ("substitute a fixed
/// `[u8; 16]`") without giving up JSON readability for telemetry/interop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EstopSource {
    bytes: [u8; 16],
    len: u8,
}

impl EstopSource {
    pub fn new(s: &str) -> Self {
        let mut bytes = [0u8; 16];
        let n = s.len().min(16);
        bytes[..n].copy_from_slice(&s.as_bytes()[..n]);
        Self {
            bytes,
            len: n as u8,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }
}

impl Serialize for EstopSource {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EstopSource {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = EstopSource;

            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("a string of at most 16 bytes")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<EstopSource, E> {
                Ok(EstopSource::new(v))
            }

            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> Result<EstopSource, E> {
                Ok(EstopSource::new(v))
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct VcuState {
    #[serde(rename = "steer")]
    pub steer_deg: f32,
    pub throttle: f32,
    pub mode: Mode,
    pub estop: bool,
    /// Milliseconds since the last *accepted* command.
    pub link_ms: u32,
    /// Last accepted command's seq.
    pub seq: u32,
    pub uptime_ms: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VcuStatus {
    Online,
    Offline,
}

// --- Serialization helpers -----------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    Encode,
    Decode,
    TooLarge,
}

/// Encodes `value` as JSON into `buf`, returning the used length.
pub fn encode<T: Serialize>(value: &T, buf: &mut [u8]) -> Result<usize, ProtocolError> {
    serde_json_core::to_slice(value, buf).map_err(|e| match e {
        serde_json_core::ser::Error::BufferFull => ProtocolError::TooLarge,
        _ => ProtocolError::Encode,
    })
}

/// Decodes JSON from `buf`, ignoring any trailing bytes.
pub fn decode<'a, T: Deserialize<'a>>(buf: &'a [u8]) -> Result<T, ProtocolError> {
    serde_json_core::from_slice(buf)
        .map(|(value, _rest)| value)
        .map_err(|_| ProtocolError::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(value: T)
    where
        T: Serialize + for<'a> Deserialize<'a> + PartialEq + core::fmt::Debug,
    {
        let mut buf = [0u8; 256];
        let len = encode(&value, &mut buf).expect("encode");
        let decoded: T = decode(&buf[..len]).expect("decode");
        assert_eq!(value, decoded);
    }

    #[test]
    fn round_trip_drive_command() {
        round_trip(DriveCommand {
            steer_deg: 12.5,
            throttle: 0.45,
            seq: 7,
        });
    }

    #[test]
    fn round_trip_mode_request() {
        round_trip(ModeRequest { mode: Mode::Auto });
    }

    #[test]
    fn round_trip_estop_command() {
        round_trip(EstopCommand {
            assert: true,
            source: EstopSource::new("joystick"),
        });
    }

    #[test]
    fn round_trip_vcu_state() {
        round_trip(VcuState {
            steer_deg: -10.0,
            throttle: 0.2,
            mode: Mode::Manual,
            estop: false,
            link_ms: 12,
            seq: 99,
            uptime_ms: 123456,
        });
    }

    #[test]
    fn round_trip_vcu_status() {
        round_trip(VcuStatus::Online);
        round_trip(VcuStatus::Offline);
    }

    #[test]
    fn clamping_out_of_range() {
        let cmd = DriveCommand {
            steer_deg: 999.0,
            throttle: 5.0,
            seq: 1,
        }
        .clamped();
        assert_eq!(cmd.steer_deg, STEER_MAX_DEG);
        assert_eq!(cmd.throttle, THROTTLE_MAX);

        let cmd = DriveCommand {
            steer_deg: -999.0,
            throttle: -5.0,
            seq: 1,
        }
        .clamped();
        assert_eq!(cmd.steer_deg, STEER_MIN_DEG);
        assert_eq!(cmd.throttle, THROTTLE_MIN);
    }

    #[test]
    fn nan_rejection() {
        let cmd = DriveCommand {
            steer_deg: f32::NAN,
            throttle: f32::INFINITY,
            seq: 1,
        }
        .clamped();
        assert_eq!(cmd.steer_deg, 0.0);
        assert_eq!(cmd.throttle, 0.0);
    }

    #[test]
    fn mode_priority_ordering() {
        assert!(Mode::Estop.priority() > Mode::Manual.priority());
        assert!(Mode::Manual.priority() > Mode::Auto.priority());
    }

    #[test]
    fn json_key_stability_drive_command() {
        let cmd = DriveCommand {
            steer_deg: 12.5,
            throttle: 0.45,
            seq: 7,
        };
        let mut buf = [0u8; 128];
        let len = encode(&cmd, &mut buf).expect("encode");
        assert_eq!(
            core::str::from_utf8(&buf[..len]).unwrap(),
            r#"{"steer":12.5,"throttle":0.45,"seq":7}"#
        );
    }
}
