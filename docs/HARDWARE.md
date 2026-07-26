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
  7.4-12V battery / bench supply
        |
        +--------------> L298N  +12V  (motor power)
        |
        +--> 5-6V BEC/UBEC (>=3A) --> MG996R servo V+
                                          |
  USB from Mac --> ESP32 (logic only)     |
        |                                 |
        +---------- COMMON GROUND --------+
                  (ESP32 GND + L298N GND + BEC GND
                   MUST all be tied together)
```

**Common ground is mandatory.** PWM signals are referenced to ground; without
a shared ground the servo and motor driver see garbage.

## L298N wiring (one board drives both motors)

| L298N pin | Connect to | Notes |
|---|---|---|
| `+12V` | Motor supply positive (7.4-12V) | L298N drops ~2V; use >=7V for 5V motors |
| `GND` | Common ground | Tie to ESP32 GND |
| `+5V` | — | Output only if 5V-regulator jumper is ON. Do **not** feed the servo from here. |
| `ENA` | ESP32 GPIO19 | PWM, speed motor A. **Remove the ENA jumper.** |
| `IN1` | ESP32 GPIO21 | Direction A |
| `IN2` | ESP32 GPIO22 | Direction A |
| `ENB` | ESP32 GPIO23 | PWM, speed motor B. **Remove the ENB jumper.** |
| `IN3` | ESP32 GPIO25 | Direction B |
| `IN4` | ESP32 GPIO26 | Direction B |
| `OUT1/OUT2` | Motor A terminals | |
| `OUT3/OUT4` | Motor B terminals | |

Direction truth table per channel: `IN1=1, IN2=0` -> forward;
`IN1=0, IN2=1` -> reverse; `IN1=IN2` -> brake/stop. Speed comes from the PWM
duty on `EN`.

> The **jumpers on ENA/ENB must be removed** or the driver runs at fixed full
> speed and ignores your PWM entirely.

## MG996R servo

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

- **GPIO 6-11**: connected to internal flash. Never use.
- **GPIO 34, 35, 36, 39**: input-only. Never drive as outputs.
- **ADC2 pins are unusable while WiFi is active.** Use ADC1 (GPIO 32-39) only.
- **GPIO 0, 2, 12, 15** are strapping pins. GPIO2 is acceptable for the
  onboard LED; avoid GPIO12 entirely (must be low at boot).

## S0 bench setup

For S0 nothing needs to be powered or wired — this stage is software only.
