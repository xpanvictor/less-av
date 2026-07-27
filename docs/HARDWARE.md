# Hardware

Canonical wiring reference. S0 is software-only — nothing needs to be
powered or wired for this stage. Wire the rig during S2. Documenting it now
means the pin constants in `config.rs` are decided before any driver code is
written.

## Power — read this first

**The MG996R can draw ~2.5 A when stalled.** Powering it from the ESP32's 5V
pin or from the L298N's onboard 5V regulator will brown out the ESP32 and
cause apparently random resets. This is the single most common failure mode
in this build.

Required power topology:

```
12V pack
   |
   +------------------> L298N #1  +12V  (rear motor power)
   |
   +------------------> L298N #2  +12V  (front motor power)

ESP32 powered separately via USB from Mac.

Common ground rail on breadboard:
   12V pack GND --+
   L298N #1 GND --+--> breadboard GND rail <-- ESP32 GND pin
   L298N #2 GND --+

CRITICAL: all four GND points must be on the same rail.
Missing common ground = PWM signals float = motors ignore commands.

Do NOT power the ESP32 from the L298N 5V regulator pin.
The L298N onboard 5V regulator cannot supply enough current for
WiFi TX bursts and will brownout the ESP32.
```

**Common ground is mandatory.** PWM signals are referenced to ground; without
a shared ground the motor drivers (and, later, the servo) see garbage.

> **Servo status (as of S2.5): not wired.** The BEC required to power the
> MG996R safely (see the power warning above) is not yet available on the
> bench rig. Steering is done via front-motor differential speed instead --
> see "Four-motor wiring" below and `vcu::steering`. The MG996R wiring table
> further down remains the reference for when the BEC arrives;
> `vcu::drivers::servo` is a compile-only stub until then.

## Four-motor wiring (S2.5+)

Board confirmed: **ESP32 DevKitC v1, 30-pin, WROOM module (no PSRAM).** Two
independent L298N boards drive four motors: board #1 is drive (rear, equal
throttle both sides always), board #2 is differential steering (front, speed
differential between the two sides). `vcu::steering::DifferentialSteering`
owns the front pair; `vcu::drivers::motor::Motors` owns the rear pair. See
`vcu/src/control/actuators.rs` for how a single `DriveCommand` drives both.

> **GPIO16/17 correction:** these are electrically valid general-purpose
> pins on the WROOM module, but on this specific 30-pin board they are *not
> broken out to the header* -- there's no physical pin to wire to. Front-left
> `IN1`/`IN2` moved to GPIO32/33 instead (plain ADC1-capable GPIOs, fine as
> digital outputs). If you're wiring a different 30-pin board where GPIO16/17
> *are* exposed, either pin choice works -- just keep `config.rs` matching
> whatever you actually wire.

```
ESP32 pin    ->    L298N terminal    Notes
-----------------------------------------------------------
-- L298N #1 (rear, drive) --------------------------------
GPIO19       ->    ENA               Remove ENA jumper first
GPIO21       ->    IN1
GPIO22       ->    IN2
GPIO23       ->    ENB               Remove ENB jumper first
GPIO25       ->    IN3
GPIO26       ->    IN4
-- L298N #2 (front, differential) -------------------------
GPIO4        ->    ENA               Remove ENA jumper first
GPIO32       ->    IN1               not GPIO16 -- see note above
GPIO33       ->    IN2               not GPIO17 -- see note above
GPIO13       ->    ENB               Remove ENB jumper first
GPIO14       ->    IN3
GPIO15       ->    IN4               strapping pin, safe here --
                                     see pin constraints below
-- Motors ---------------------------------------------------
Rear  left   ->    L298N #1  OUT1/OUT2
Rear  right  ->    L298N #1  OUT3/OUT4
Front left   ->    L298N #2  OUT1/OUT2
Front right  ->    L298N #2  OUT3/OUT4
```

Matches `vcu/src/config.rs`: `PIN_REAR_L_EN/IN1/IN2` = GPIO19/21/22,
`PIN_REAR_R_EN/IN1/IN2` = GPIO23/25/26, `PIN_FRONT_L_EN/IN1/IN2` =
GPIO4/32/33, `PIN_FRONT_R_EN/IN1/IN2` = GPIO13/14/15. LEDC channels 0-3 are
assigned one per motor (`LEDC_CH_REAR_L/R`, `LEDC_CH_FRONT_L/R`) -- all four
share one LEDC timer at 1kHz/10-bit (`MOTOR_PWM_HZ`/`MOTOR_PWM_BITS`).

Direction truth table, same for all four channels: `IN1=1, IN2=0` -> forward;
`IN1=0, IN2=1` -> reverse; `IN1=IN2` -> brake/stop. Speed comes from the PWM
duty on `EN`.

> The **jumpers on ENA/ENB must be removed on both boards** (all four
> channels) or the driver runs at fixed full speed and ignores your PWM
> entirely.

### Pre-power checklist

Before connecting the 12V pack, verify with a multimeter:

- [ ] ENA jumper removed from L298N #1
- [ ] ENB jumper removed from L298N #1
- [ ] ENA jumper removed from L298N #2
- [ ] ENB jumper removed from L298N #2
- [ ] Continuity: ESP32 GND <-> L298N #1 GND (should beep)
- [ ] Continuity: ESP32 GND <-> L298N #2 GND (should beep)
- [ ] Continuity: GPIO19 <-> L298N #1 ENA (should beep)
- [ ] Continuity: GPIO4 <-> L298N #2 ENA (should beep)
- [ ] No continuity between +12V rail and GND rail (would indicate a short)

## MG996R servo (reference only -- not wired until a BEC is available)

| Servo wire | Connect to |
|---|---|
| Brown / black | Common ground |
| Red | BEC 5-6V output (**not** ESP32, **not** L298N 5V) |
| Orange / yellow | ESP32 GPIO18 |

Signal: 50 Hz PWM, 1000 us = full left, 1500 us = centre, 2000 us = full
right.

## Joystick node (ESP32-CAM, S4)

**Board: ESP32-CAM (AI-Thinker variant).** Different board from the VCU's
plain DevKitC -- this one has a camera module and SD card slot wired to
several GPIOs internally, so its pin constraints are stricter and different
from the VCU's. No ESTOP button on this node; ESTOP is dashboard-only (S5).

```
Thumbstick pin     ->    ESP32-CAM pin
--------------------------------------
VCC                ->    3V3
GND                ->    GND
VRx (X / steer)    ->    GPIO34
VRy (Y / throttle) ->    GPIO35
```

| Component | ESP32-CAM pin | Notes |
|---|---|---|
| Pot X wiper (steer) | GPIO34 | ADC1_CH6, input-only |
| Pot Y wiper (throttle) | GPIO35 | ADC1_CH7, input-only |
| Pot ends | 3V3 and GND | |
| Heartbeat LED | GPIO33 (onboard) | Active LOW -- already populated, no wiring needed |

### ESP32-CAM pin map

The camera module and SD card slot use several GPIOs internally even though
this firmware never drives the camera. Do not repurpose any pin marked NO
below without first confirming it's actually unused on your specific board.

```
Pin     Internal use                    Available for joystick?
-----------------------------------------------------------------
GPIO0   Camera (PWDN) / boot mode       NO -- boot strapping pin
GPIO1   UART TX                         NO -- serial monitor
GPIO3   UART RX                         NO -- serial monitor
GPIO2   SD card D0                      NO
GPIO4   SD card D1 / onboard flash LED  NO
GPIO12  SD card D2                      NO
GPIO13  SD card D3                      YES if SD not inserted
GPIO14  SD card CLK                     NO
GPIO15  SD card CMD                     NO
GPIO16  PSRAM (if present)              NO
GPIO17  PSRAM (if present)              NO
GPIO32  Camera (XCLK alt)               YES
GPIO33  Onboard red LED (active LOW)    YES -- used for heartbeat
GPIO34  Input only, ADC1_CH6            YES -- VRx (X axis / steer)
GPIO35  Input only, ADC1_CH7            YES -- VRy (Y axis / throttle)
GPIO25  Camera (VSYNC)                  NO when camera active
GPIO26  Camera (HREF)                   NO when camera active
GPIO27  Camera (PCLK)                   NO when camera active
```

Matches `joystick/src/config.rs`: `PIN_AXIS_X` = GPIO34, `PIN_AXIS_Y` =
GPIO35, `PIN_LED_HEARTBEAT` = GPIO33.

## ESP32 pin constraints (do not violate)

**VCU only.** Cross-checked against our confirmed board: ESP32 DevKitC v1,
30-pin, WROOM module (no PSRAM). The joystick's ESP32-CAM is a different
board with its own constraints -- see the pin map above instead.

- **GPIO 6-11**: connected to internal flash. Never use.
- **GPIO 34, 35, 36, 39**: input-only. Never drive as outputs.
- **GPIO 1, 3**: UART TX/RX. Leave free for the serial monitor.
- **ADC2 pins are unusable while WiFi is active** for analog reads (doesn't
  matter for digital PWM output, e.g. GPIO4 as a motor EN pin). Use ADC1
  (GPIO 32-39) for any future analog input.
- **GPIO 0, 2, 12, 15** are strapping pins -- usable as outputs *after* boot,
  but must not be held in the wrong state *at* boot. GPIO2 is the heartbeat
  LED. GPIO12 must be LOW at boot; avoid it. GPIO15 must also be LOW at boot,
  which it is here (`PIN_FRONT_R_IN2`) since an L298N direction input reads
  LOW on power-on/reset by default -- confirmed safe, but if that pin is ever
  repurposed, re-check this constraint.
- **GPIO 16, 17**: electrically valid, non-PSRAM-conflicting general-purpose
  pins on the WROOM module -- but **not physically broken out to the header
  on this specific 30-pin board**. Don't wire to them; there's nothing to
  connect to. Used GPIO32/33 for front-left `IN1`/`IN2` instead. If your
  board variant does expose 16/17, they're a fine alternative -- just keep
  `config.rs` matching whatever you actually wire.
- **Free pins remaining** after the current allocation: GPIO 0, 5, 27 (0 and
  5 are strapping pins, avoid unless needed); input-only GPIO 36, 39 also
  remain free. Available for S4+ (joystick ESTOP button, camera, etc).

## S0 bench setup

For S0 nothing needs to be powered or wired — this stage is software only.
