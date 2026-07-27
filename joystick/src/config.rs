//! Pin assignments and hardware limits for the joystick node. Constants
//! only — see docs/HARDWARE.md for wiring rationale. No logic here.
//!
//! Board: ESP32-CAM (AI-Thinker). No ESTOP button on this node -- ESTOP is
//! S5 dashboard-only. GPIO34/35 and GPIO33 are the only pins used; every
//! other ESP32-CAM pin is reserved for the camera/SD card hardware even
//! though the camera itself isn't driven by this firmware.

// --- Pins ---------------------------------------------------------------
//
// PIN_AXIS_X and PIN_AXIS_Y are both ADC1 (GPIO 32-39, input-only). ADC2 is
// unusable while WiFi is active on ESP32 -- do not move these without
// checking that. Both are free on the ESP32-CAM (not used by camera/SD).

/// Steering axis (ADC1_CH6).
pub const PIN_AXIS_X: u8 = 34;
/// Throttle axis (ADC1_CH7).
pub const PIN_AXIS_Y: u8 = 35;
/// Onboard red LED on the ESP32-CAM. Active LOW: set LOW to turn on.
pub const PIN_LED_HEARTBEAT: u8 = 33;

// --- ADC --------------------------------------------------------------

pub const ADC_BITS: u8 = 12;
pub const ADC_MAX: u16 = 4095;
/// Nominal centre count; overridden by boot calibration in `calibration.rs`.
pub const ADC_CENTER_NOMINAL: u16 = 2048;
/// Raw counts on each side of centre to treat as zero. 120 counts ≈ 3% of
/// full range. Tune after calibration if the rest reading still drifts.
pub const DEADZONE_COUNTS: u16 = 120;

// --- Axis orientation ---------------------------------------------------
//
// Physical thumbstick axes may be inverted relative to the expected
// direction. Determined after first physical test (S4 T4/T5) -- one-line
// change, then rebuild.

/// Flip if pushing the stick right produces a negative `steer_deg`.
pub const INVERT_X: bool = false;
/// Flip if pushing the stick forward produces a negative `throttle`.
pub const INVERT_Y: bool = false;

// --- Publish rate ---------------------------------------------------------

/// Matches `shared::CMD_PUBLISH_HZ`.
pub const CMD_PUBLISH_HZ: u32 = 50;

// --- MQTT -----------------------------------------------------------------

pub const MQTT_CLIENT_ID: &str = "less-joystick";
pub const MQTT_BROKER_PORT: u16 = 1883;

/// WiFi/broker credentials are compile-time env vars, never hardcoded.
/// Set WIFI_SSID, WIFI_PASSWORD, MQTT_BROKER_HOST before building — see README.md.
pub const WIFI_SSID: &str = env!("WIFI_SSID");
pub const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
pub const MQTT_BROKER_HOST: &str = env!("MQTT_BROKER_HOST");
