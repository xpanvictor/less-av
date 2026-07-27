# Architecture

## Nodes

| Node | Hardware | Role |
|---|---|---|
| **VCU** | ESP32 | Vehicle Control Unit. Owns actuators. Sole authority on mode + safety. |
| **Joystick** | ESP32-CAM + 2-axis pot | Manual drive-by-wire input. No ESTOP button -- ESTOP is dashboard-only (S5). |
| **Dashboard** | iPad browser | Manual control + telemetry over MQTT-WebSocket. |
| **Camera** | ESP32-CAM | MJPEG video stream (S6). |
| **Autonomy** | Mac Mini (Python) | Publishes autonomous commands (S7). |
| **Broker** | Mac Mini (Mosquitto) | MQTT backbone, ports 1883 + 9001. |

All five nodes and the broker share one WiFi LAN.

```
                        +-------------------+
                        |   Broker (Mac)    |
                        | Mosquitto 1883/   |
                        |         9001 (ws) |
                        +---------+---------+
                                  |
        +---------------+--------+--------+---------------+
        |               |                 |               |
  +-----+-----+   +-----+-----+    +------+-----+   +------+------+
  |    VCU    |   | Joystick  |    | Dashboard  |   |  Autonomy   |
  |  (ESP32)  |   |  (ESP32)  |    | (iPad, ws) |   | (Mac Mini)  |
  | actuators |   | manual in |    | manual +   |   | closes the  |
  | + safety  |   |           |    | telemetry  |   | loop (S7)   |
  +-----+-----+   +-----------+    +------------+   +-------------+
        |
  +-----+-----+
  |  Camera   |
  | (ESP32-CAM)
  |  MJPEG    |
  +-----------+
```

**The VCU is the sole actuator authority.** Every other node is an advisor
publishing requests (drive commands, mode requests, ESTOP asserts) onto MQTT
topics; only the VCU decides what the servo and motors actually do. This
property must never be diluted by a later stage — no other node is ever
allowed to drive the actuators directly.

## Safety invariants

1. **Deadman timeout.** If the VCU has not accepted a command within
   `CMD_TIMEOUT_MS` (300 ms), it falls back to the safe state
   (`DriveCommand::safe()`: steer 0, throttle 0).
2. **Latched ESTOP.** Once asserted (`less/v1/estop`, `assert: true`), the
   VCU remains in `Mode::Estop` until an explicit clear is accepted. ESTOP
   state is retained so a reconnecting node immediately sees it.
3. **Absolute mode priority.** `Mode::priority()` orders `Estop (3) > Manual
   (2) > Auto (1)`. A higher-priority mode request always wins arbitration,
   regardless of arrival order.
4. **WiFi-loss → safe state.** If the VCU loses its broker connection, it
   must fall back to the safe state exactly as if the deadman timer expired
   — a silent link is not a license to keep the last command running.

## Known limitations

- **Anonymous MQTT.** `allow_anonymous true` — any device on the LAN can
  publish/subscribe to any topic. Acceptable for a closed demo LAN; must be
  revisited before this leaves the bench.
- **No TLS.** All MQTT traffic (1883 and 9001) is plaintext.
- **No authentication.** No client IDs are verified; anyone with LAN access
  can impersonate any node.
