//! Pin assignments and hardware limits for the VCU node. Constants only — see
//! docs/HARDWARE.md for wiring rationale. No logic here.

// --- Pins ---------------------------------------------------------------

pub const PIN_SERVO: u8 = 18;
pub const PIN_MOTOR_ENA: u8 = 19;
pub const PIN_MOTOR_IN1: u8 = 21;
pub const PIN_MOTOR_IN2: u8 = 22;
pub const PIN_MOTOR_ENB: u8 = 23;
pub const PIN_MOTOR_IN3: u8 = 25;
pub const PIN_MOTOR_IN4: u8 = 26;
pub const PIN_LED_HEARTBEAT: u8 = 2;
/// Input-only pin. Reserved for a wheel encoder; unused until later.
pub const PIN_ENCODER_L: u8 = 34;
/// Input-only pin. Reserved for a wheel encoder; unused until later.
pub const PIN_ENCODER_R: u8 = 35;

// --- Servo / motor PWM ----------------------------------------------------

pub const SERVO_PWM_HZ: u32 = 50;
pub const SERVO_MIN_US: u32 = 1000;
pub const SERVO_CENTER_US: u32 = 1500;
pub const SERVO_MAX_US: u32 = 2000;

pub const MOTOR_PWM_HZ: u32 = 1000;

// --- MQTT -----------------------------------------------------------------

pub const MQTT_CLIENT_ID: &str = "less-vcu";
pub const MQTT_BROKER_PORT: u16 = 1883;

/// WiFi/broker credentials are compile-time env vars, never hardcoded.
/// Set WIFI_SSID, WIFI_PASSWORD, MQTT_BROKER_HOST before building — see README.md.
pub const WIFI_SSID: &str = env!("WIFI_SSID");
pub const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
pub const MQTT_BROKER_HOST: &str = env!("MQTT_BROKER_HOST");
