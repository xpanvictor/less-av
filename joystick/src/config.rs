//! Pin assignments and hardware limits for the joystick node. Constants
//! only — see docs/HARDWARE.md for wiring rationale. No logic here.

// --- Pins ---------------------------------------------------------------
//
// PIN_AXIS_X and PIN_AXIS_Y are both ADC1 (GPIO 32-39). ADC2 is unusable
// while WiFi is active on ESP32 — do not move these without checking that.

/// Steering axis (ADC1_CH6).
pub const PIN_AXIS_X: u8 = 34;
/// Throttle axis (ADC1_CH7).
pub const PIN_AXIS_Y: u8 = 35;
/// ESTOP button, active-low, internal pull-up.
pub const PIN_ESTOP_BTN: u8 = 27;
pub const PIN_LED_HEARTBEAT: u8 = 2;

// --- ADC --------------------------------------------------------------

pub const ADC_MAX: u16 = 4095;
pub const DEADZONE_COUNTS: u16 = 120;

// --- MQTT -----------------------------------------------------------------

pub const MQTT_CLIENT_ID: &str = "less-joystick";
pub const MQTT_BROKER_PORT: u16 = 1883;

/// WiFi/broker credentials are compile-time env vars, never hardcoded.
/// Set WIFI_SSID, WIFI_PASSWORD, MQTT_BROKER_HOST before building — see README.md.
pub const WIFI_SSID: &str = env!("WIFI_SSID");
pub const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
pub const MQTT_BROKER_HOST: &str = env!("MQTT_BROKER_HOST");
