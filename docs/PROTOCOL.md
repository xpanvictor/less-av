# Protocol

Single source of truth: [`shared/src/lib.rs`](../shared/src/lib.rs). Every
constant and type here must match that crate exactly — the crate's unit tests
(including a JSON key-stability tripwire) exist to guarantee it. If this
document and the crate ever disagree, the crate is right and this document is
stale.

Wire format is JSON (via `serde-json-core`), chosen over a binary format
because interop with the JS dashboard and the Python autonomy node is worth
more than byte efficiency here.

## Topics

All topics are rooted at `less/v1` (`PROTOCOL_VERSION = 1`). A version bump
means changing `TOPIC_ROOT` in `shared` — every topic constant is derived from
it, so that's a one-line change.

| Constant | Topic string | Direction | Retained | QoS |
|---|---|---|---|---|
| `TOPIC_CMD_MANUAL` | `less/v1/cmd/manual` | joystick/dashboard → VCU | no | 0 |
| `TOPIC_CMD_AUTO` | `less/v1/cmd/auto` | autonomy → VCU | no | 0 |
| `TOPIC_MODE_REQUEST` | `less/v1/mode/request` | any → VCU | no | 1 |
| `TOPIC_MODE_CURRENT` | `less/v1/mode/current` | VCU → all | **yes** | 1 |
| `TOPIC_ESTOP` | `less/v1/estop` | any → VCU | **yes** | 1 |
| `TOPIC_VCU_STATE` | `less/v1/vcu/state` | VCU → all | no | 0 |
| `TOPIC_VCU_STATUS` | `less/v1/vcu/status` | VCU → all (LWT) | **yes** | 1 |

`TOPIC_ALL` (`less/v1/#`) is a debugging subscription wildcard, not a publish
target.

## Message types

### `DriveCommand` — `cmd/manual`, `cmd/auto`

| Field | Type | JSON key | Range / meaning |
|---|---|---|---|
| `steer_deg` | `f32` | `steer` | Steering angle, degrees. Negative = left. Clamped to [`STEER_MIN_DEG`, `STEER_MAX_DEG`]. |
| `throttle` | `f32` | `throttle` | Normalized. `-1.0` full reverse … `1.0` full forward. Clamped to [`THROTTLE_MIN`, `THROTTLE_MAX`]. |
| `seq` | `u32` | `seq` | Monotonic per-publisher counter. Wraps. |

`DriveCommand::clamped()` also replaces NaN/infinity with `0.0` before the
range clamp — a NaN reaching the servo math produces undefined PWM.

Example (`less/v1/cmd/manual`):

```
mosquitto_pub -h localhost -t less/v1/cmd/manual -m '{"steer":12.5,"throttle":0.45,"seq":7}'
```

Example (`less/v1/cmd/auto`):

```
mosquitto_pub -h localhost -t less/v1/cmd/auto -m '{"steer":-5.0,"throttle":0.3,"seq":42}'
```

### `Mode` (enum)

Variants `Estop`, `Manual`, `Auto`, serialized as uppercase strings:
`"ESTOP"`, `"MANUAL"`, `"AUTO"`. `Mode::priority()` returns
`Estop = 3, Manual = 2, Auto = 1` — higher wins in arbitration (S3).

### `ModeRequest` — `mode/request`, `mode/current`

| Field | Type | JSON key |
|---|---|---|
| `mode` | `Mode` | `mode` |

`mode/current` carries the same shape, published retained by the VCU so a
reconnecting node immediately learns the active mode.

Example (`less/v1/mode/request`):

```
mosquitto_pub -h localhost -t less/v1/mode/request -m '{"mode":"AUTO"}'
```

Example (`less/v1/mode/current`, retained):

```
mosquitto_pub -h localhost -t less/v1/mode/current -m '{"mode":"MANUAL"}' -r
```

### `EstopCommand` — `estop`

| Field | Type | JSON key | Meaning |
|---|---|---|---|
| `assert` | `bool` | `assert` | `true` = engage ESTOP, `false` = request clear |
| `source` | fixed string, max 16 bytes | `src` | Who asserted it, for telemetry |

Example (`less/v1/estop`, retained):

```
mosquitto_pub -h localhost -t less/v1/estop -m '{"assert":true,"src":"joystick"}' -r
```

### `VcuState` — `vcu/state`

| Field | Type | JSON key | Meaning |
|---|---|---|---|
| `steer_deg` | `f32` | `steer` | Currently applied steering |
| `throttle` | `f32` | `throttle` | Currently applied throttle |
| `mode` | `Mode` | `mode` | Active mode |
| `estop` | `bool` | `estop` | Latched ESTOP state |
| `link_ms` | `u32` | `link_ms` | ms since last *accepted* command |
| `seq` | `u32` | `seq` | Last accepted command's seq |
| `uptime_ms` | `u32` | `uptime_ms` | VCU uptime |

Example (`less/v1/vcu/state`):

```
mosquitto_pub -h localhost -t less/v1/vcu/state -m '{"steer":-10.0,"throttle":0.2,"mode":"MANUAL","estop":false,"link_ms":12,"seq":99,"uptime_ms":123456}'
```

### `VcuStatus` (enum) — `vcu/status`

Variants `Online`, `Offline`, serialized as lowercase strings `"online"` /
`"offline"`. Published retained; `"offline"` is the MQTT Last Will payload so
any node can detect a VCU that vanished without a clean disconnect.

Example (`less/v1/vcu/status`, retained):

```
mosquitto_pub -h localhost -t less/v1/vcu/status -m '"online"' -r
```

## Limits

| Constant | Value | Rationale |
|---|---|---|
| `STEER_MAX_DEG` | `35.0` | Bench servo safe travel, symmetric |
| `STEER_MIN_DEG` | `-35.0` | |
| `THROTTLE_MAX` | `1.0` | |
| `THROTTLE_MIN` | `-1.0` | |
| `CMD_TIMEOUT_MS` | `300` | Deadman: no command in 300ms → safe state |
| `STATE_PUBLISH_HZ` | `10` | VCU telemetry rate |
| `CMD_PUBLISH_HZ` | `50` | Joystick/autonomy command rate |
| `MAX_PAYLOAD_CMD` | `128` | JSON buffer sizing |
| `MAX_PAYLOAD_STATE` | `256` | |

**Symmetric vs Ackermann steering.** The real vehicle's Ackermann geometry
gives asymmetric steering limits (inner wheel ~40°, outer wheel ~26.1°). The
bench rig in S0–S7 has no chassis, so `STEER_MAX_DEG`/`STEER_MIN_DEG` are a
symmetric ±35° clamp instead. When a chassis exists, replace this single pair
with the asymmetric inner/outer pair — don't forget this constraint is
currently a simplification, not the real vehicle's geometry.
