//! Pin assignments and hardware limits for the VCU node. Constants only — see
//! docs/HARDWARE.md for wiring rationale. No logic here.

// --- Pins ---------------------------------------------------------------

/// Not currently wired -- no BEC available yet. See `steering::servo`.
pub const PIN_SERVO: u8 = 18;

// Board confirmed: ESP32 DevKitC v1, 30-pin, WROOM module (no PSRAM). The
// WROVER/PSRAM caveat from S2.5 doesn't apply. However, GPIO16/17 -- while
// electrically valid general-purpose pins on the WROOM module itself -- are
// not physically broken out to the header on this particular board, so they
// can't be wired to at all. Moved front-left IN1/IN2 to GPIO32/33 instead
// (see PIN_FRONT_L_IN1/IN2 below).

/// L298N #1 -- rear (drive): equal throttle both sides, always.
pub const PIN_REAR_L_EN: u8 = 19;
pub const PIN_REAR_L_IN1: u8 = 21;
pub const PIN_REAR_L_IN2: u8 = 22;
pub const PIN_REAR_R_EN: u8 = 23;
pub const PIN_REAR_R_IN1: u8 = 25;
pub const PIN_REAR_R_IN2: u8 = 26;

/// L298N #2 -- front (differential steering).
pub const PIN_FRONT_L_EN: u8 = 4;
/// GPIO16/17 are not exposed on this board's header -- see the board note
/// above. GPIO32/33 are plain ADC1-capable GPIOs, fine as digital outputs.
pub const PIN_FRONT_L_IN1: u8 = 32;
pub const PIN_FRONT_L_IN2: u8 = 33;
pub const PIN_FRONT_R_EN: u8 = 13;
pub const PIN_FRONT_R_IN1: u8 = 14;
/// Strapping pin, but safe as an output here: it only needs to be LOW at
/// boot, and an L298N direction input is LOW on power-on/reset by default.
pub const PIN_FRONT_R_IN2: u8 = 15;

/// LEDC channel assignments -- must not overlap with each other or with any
/// other LEDC use (currently none; servo is a stub and does not use LEDC).
pub const LEDC_CH_REAR_L: u8 = 0;
pub const LEDC_CH_REAR_R: u8 = 1;
pub const LEDC_CH_FRONT_L: u8 = 2;
pub const LEDC_CH_FRONT_R: u8 = 3;

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

pub const MOTOR_PWM_HZ: u32 = 1_000;
/// Duty-cycle resolution for all four motor LEDC channels. Max duty is
/// `2^MOTOR_PWM_BITS - 1` = 1023.
pub const MOTOR_PWM_BITS: u8 = 10;

// --- MQTT -----------------------------------------------------------------

pub const MQTT_CLIENT_ID: &str = "less-vcu";
pub const MQTT_BROKER_PORT: u16 = 1883;

/// WiFi/broker credentials are compile-time env vars, never hardcoded.
/// Set WIFI_SSID, WIFI_PASSWORD, MQTT_BROKER_HOST before building — see README.md.
pub const WIFI_SSID: &str = env!("WIFI_SSID");
pub const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
pub const MQTT_BROKER_HOST: &str = env!("MQTT_BROKER_HOST");
