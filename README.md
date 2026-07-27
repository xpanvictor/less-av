# LESS

A modular autonomy platform. This repo is the demonstrator: a bench rig (no
chassis) that proves an end-to-end autonomy control path across five nodes and
one MQTT broker, all on the same WiFi LAN. The VCU is the sole authority on
mode and safety; every other node is an advisor publishing requests.

| Node | Hardware | Role |
|---|---|---|
| **VCU** | ESP32 | Vehicle Control Unit. Owns actuators. Sole authority on mode + safety. |
| **Joystick** | ESP32-CAM + 2-axis pot | Manual drive-by-wire input. No ESTOP button -- ESTOP is dashboard-only (S5). |
| **Dashboard** | iPad browser | Manual control + telemetry over MQTT-WebSocket. |
| **Camera** | ESP32-CAM | MJPEG video stream (S6). |
| **Autonomy** | Mac Mini (Python) | Publishes autonomous commands (S7). |
| **Broker** | Mac Mini (Mosquitto) | MQTT backbone, ports 1883 + 9001. |

## Staging map

| Stage | Deliverable |
|---|---|
| **S0** | Workspace, `shared` contract crate, broker config, docs |
| **S1** | VCU connects to WiFi + broker, publishes status/heartbeat |
| S2 | Servo + motor driven by `cmd/manual` |
| S3 | Mode arbitration, deadman timeout, latched ESTOP |
| S4 | Joystick node publishes real input |
| S5 | iPad dashboard |
| S6 | ESP32-CAM MJPEG stream |
| S7 | Autonomy node closes the loop |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/PROTOCOL.md](docs/PROTOCOL.md),
and [docs/HARDWARE.md](docs/HARDWARE.md) for details.

## Prerequisites

- [`espup`](https://github.com/esp-rs/espup) — installs and manages the Xtensa
  Rust toolchain (`xtensa-esp32-none-elf`) used by `vcu` and `joystick`.
- Rust, via `rustup`, plus the `esp` toolchain that `espup` installs.
- Mosquitto (`brew install mosquitto`) for the broker.
- Python 3 for the `autonomy` node.

## Required environment variables

`vcu` and `joystick` read WiFi/broker credentials at compile time via `env!`
— nothing is hardcoded. Set these before building either firmware crate:

- `WIFI_SSID`
- `WIFI_PASSWORD`
- `MQTT_BROKER_HOST` — the Mac's LAN IP; `infra/run_broker.sh` prints it.

## Running the S0 acceptance tests

All commands run from the repo root.

```sh
# T1 — workspace compiles. Note: because vcu/joystick require the Xtensa
# target and shared/host tooling does not, a bare `cargo check --workspace`
# from the repo root cannot type-check all three at once (Cargo picks one
# default target per invocation). Check per-crate instead:
cargo check -p shared
(cd vcu && WIFI_SSID=x WIFI_PASSWORD=x MQTT_BROKER_HOST=x cargo check)
(cd joystick && WIFI_SSID=x WIFI_PASSWORD=x MQTT_BROKER_HOST=x cargo check)

# T2 — shared crate unit tests
cargo test -p shared

# T3 — shared crate builds for the firmware target
# (-Z build-std is required because shared has no prebuilt std/core for this
# target; vcu/joystick get this from their own .cargo/config.toml, but that
# config is directory-scoped and doesn't apply when building from the repo root)
cargo build -p shared --target xtensa-esp32-none-elf -Z build-std=core,alloc

# T4 — broker starts
./infra/run_broker.sh

# T5 — MQTT round trip on 1883 (in a second terminal)
mosquitto_sub -h localhost -t 'less/v1/#' -v
mosquitto_pub -h localhost -t less/v1/cmd/manual -m '{"steer":12.5,"throttle":0.45,"seq":7}'

# T6 — WebSocket listener reachable
nc -z localhost 9001 && echo "9001 OPEN"

# T7 — retained-message behaviour
mosquitto_pub -h localhost -t less/v1/mode/current -m '{"mode":"MANUAL"}' -r
mosquitto_sub -h localhost -t less/v1/mode/current -C 1
```

## Running the VCU (S1)

The VCU speaks native MQTT (v5) directly to the broker over WiFi — no bridge
process needed. With the broker already running (`./infra/run_broker.sh`) and
the ESP32 connected over USB:

```sh
cd vcu
WIFI_SSID='<your-ssid>' WIFI_PASSWORD='<your-password>' MQTT_BROKER_HOST='<mac-lan-ip>' \
  cargo run --release
```

`cargo run` flashes and opens a serial monitor (via `espflash`, configured in
`vcu/.cargo/config.toml`). Watch for the WiFi connect log, the DHCP-assigned
IP address, and `mqtt: announced online`. See `docs/PROTOCOL.md` for what's
published on `less/v1/vcu/status` and `less/v1/vcu/state`.

## Boot and arming (S3+)

**The VCU boots ESTOP-latched and in ESTOP mode.** This is intentional and
correct: the car must not move until an operator explicitly arms it, and a
power cycle must never be a way to bypass that. `less/v1/cmd/manual` is
ignored entirely until both of the following have been published:

```sh
# 1. Clear ESTOP
mosquitto_pub -h <mac-lan-ip> -t less/v1/estop -r \
  -m '{"assert":false,"src":"operator"}'

# 2. Request Manual mode
mosquitto_pub -h <mac-lan-ip> -t less/v1/mode/request \
  -m '{"mode":"MANUAL"}'

# 3. Drive commands are now accepted
mosquitto_pub -h <mac-lan-ip> -t less/v1/cmd/manual \
  -m '{"steer":0.0,"throttle":0.3,"seq":1}'
```

Two independent safety mechanisms stay active once armed:
- **Deadman timeout** — if no `cmd/manual` message arrives for
  `shared::CMD_TIMEOUT_MS` (300ms), the vehicle stops itself. Sending a
  command again resumes immediately; no re-arm needed.
- **ESTOP latch** — asserting `less/v1/estop` with `"assert":true` stops the
  vehicle immediately and *stays* stopped, ignoring all drive commands, until
  an explicit `"assert":false` clear is published. A deadman timeout does
  not clear ESTOP, and clearing ESTOP does not by itself re-select Manual
  mode -- both steps above are required to re-arm after an ESTOP.

## Running the joystick (S4)

The joystick is a second ESP32 (ESP32-CAM board) reading a two-axis
potentiometer thumbstick and publishing `DriveCommand`s to `less/v1/cmd/manual`
at 50Hz. It has no knowledge of modes or ESTOP -- it's a pure input publisher;
the VCU must already be armed (see "Boot and arming" above) for its commands
to have any effect.

```sh
cd joystick
WIFI_SSID='<your-ssid>' WIFI_PASSWORD='<your-password>' MQTT_BROKER_HOST='<mac-lan-ip>' \
  cargo run --release
```

**Do not touch the joystick for the first 500ms after power-on.** It
calibrates its centre position at boot (averaging 100 samples over 500ms) to
compensate for unit-to-unit variation in the thumbstick's resting voltage --
touching it during this window skews the calibration and leaves a persistent
steer/throttle offset at rest. Watch the serial log for
`Calibration done: center_x=... center_y=...` before handling the stick.

The onboard red LED blinks fast (~4Hz) while WiFi/MQTT is still connecting,
and slow (~1Hz) once connected -- a visual status without needing the serial
monitor.

If the physical push directions come out backwards (right push steers left,
or forward push reverses), flip `INVERT_X`/`INVERT_Y` in
`joystick/src/config.rs` and rebuild -- see that file for details.
