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
GPIO16       ->    IN1
GPIO17       ->    IN2
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
GPIO4/16/17, `PIN_FRONT_R_EN/IN1/IN2` = GPIO13/14/15. LEDC channels 0-3 are
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

## Joystick node (second ESP32)

| Component | ESP32 pin |
|---|---|
| Pot X wiper | GPIO34 |
| Pot Y wiper | GPIO35 |
| Pot ends | 3V3 and GND |
| ESTOP button | GPIO27 to GND (internal pull-up, active low) |

## ESP32 pin constraints (do not violate)

Cross-checked against our confirmed board: ESP32 DevKitC v1, 30-pin, WROOM
module (no PSRAM).

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
- **GPIO 16, 17**: free general-purpose pins on our confirmed board (no
  PSRAM). Only relevant on ESP32-WROVER boards, which use them for PSRAM SPI
  -- not applicable here, but re-verify if the board ever changes.
- **Free pins remaining** after the current allocation: GPIO 0, 5, 27, 32, 33
  (0 and 5 are strapping pins, avoid unless needed); input-only GPIO 36, 39
  also remain free. Available for S4+ (joystick ESTOP button, camera, etc).

## S0 bench setup

For S0 nothing needs to be powered or wired — this stage is software only.
