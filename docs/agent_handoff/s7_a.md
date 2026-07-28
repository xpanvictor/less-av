# S7.A Handoff Spec — Waypoint Autonomy Node

> **Read this whole document before writing any code.**
> You are implementing the autonomy node only — a Python script on the Mac.
> Do not touch any firmware crate or the dashboard.

---

## 1. Context and demo purpose

**LESS** bench-rig demonstrator. The audience is a CEO evaluating this as a
self-driving layer for an electric motor company. What must be visible and
legible to a non-technical executive:

1. The car drives itself when told to — without a human on the joystick.
2. A human can take back control instantly, at any moment.
3. If anything goes wrong, the car stops safely.
4. The system has a clear, principled architecture — not a hack.

S7.A proves all four by executing a pre-defined waypoint route autonomously
while the full safety stack (deadman, ESTOP, mode arbitration) remains active.
The sophistication of the route matters less than the clarity of the mode
switch.

### Completed stages

| Stage | Status |
|---|---|
| S0–S3 — VCU, broker, safety | ✅ |
| S4 — joystick node | ✅ build (physical tests pending) |
| S5 — iPad dashboard | ✅ |
| S6 — ESP32-CAM | deferred |
| **S7.A — waypoint autonomy node** | ← you are here |

---

## 2. What this node does

A Python script (`autonomy/main.py`) runs on the Mac and executes a
pre-defined route by publishing `DriveCommand` JSON messages to
`less/v1/cmd/auto` at 20 Hz. The VCU receives them and drives the actuators
— but only when in AUTO mode with ESTOP clear.

The node is not a perception system. It has no sensors, no camera, no model.
It is a **route executor**: a sequence of waypoints, each defined as a target
steering angle, throttle, and duration, executed in order. This is sufficient
to demonstrate the complete autonomy loop and is fully honest about what it is.

---

## 3. Deliverables

```
autonomy/
├── main.py              ← REWRITE (currently a placeholder)
├── route.py             ← CREATE (waypoint definitions)
├── requirements.txt     ← MODIFY (add paho-mqtt if not present)
└── README_autonomy.md   ← CREATE (how to run, what to expect)
```

No other files. No subpackages. Plain Python 3, no frameworks.

---

## 4. Dependencies

```
# autonomy/requirements.txt
paho-mqtt==2.1.0
```

Install:
```bash
cd autonomy
pip install -r requirements.txt
# or, if using the carla conda env:
pip install paho-mqtt
```

No other dependencies. The route executor needs only MQTT — no numpy, no
OpenCV, no simulation libraries.

---

## 5. Route definition (`autonomy/route.py`)

A waypoint is a named tuple:

```python
from typing import NamedTuple

class Waypoint(NamedTuple):
    name:        str    # human-readable label, logged on entry
    steer_deg:   float  # degrees, clamped to [-35, 35]
    throttle:    float  # [-1.0, 1.0], positive = forward
    duration_ms: int    # how long to hold this waypoint

# Clamp helper — mirrors shared::DriveCommand::clamped()
def _clamp(v, lo, hi):
    return max(lo, min(hi, v))

def validate(wp: Waypoint) -> Waypoint:
    return wp._replace(
        steer_deg = _clamp(wp.steer_deg, -35.0, 35.0),
        throttle  = _clamp(wp.throttle,  -1.0,  1.0),
    )
```

### 5.1 The demo route

Design the route for a **bench rig with wheels free-spinning** (not driving
on a surface). The behavior must be visible in the wheel speeds and directions,
not in physical displacement. It should be ~25–30 seconds total — long enough
to demonstrate, short enough to repeat twice during a demo.

```python
DEMO_ROUTE = [
    Waypoint("pause",          0.0,  0.0, 2000),  # 2s armed, not moving
    Waypoint("forward",        0.0,  0.6, 3000),  # straight ahead, 3s
    Waypoint("arc-right",     25.0,  0.5, 2500),  # right arc, 2.5s
    Waypoint("straighten",     0.0,  0.5, 1500),  # straighten, 1.5s
    Waypoint("arc-left",     -25.0,  0.5, 2500),  # left arc, 2.5s
    Waypoint("straighten",     0.0,  0.5, 1500),  # straighten, 1.5s
    Waypoint("slow",           0.0,  0.3, 1500),  # decelerate, 1.5s
    Waypoint("stop",           0.0,  0.0, 2000),  # full stop, 2s
    Waypoint("reverse",        0.0, -0.4, 2000),  # reverse, 2s
    Waypoint("stop",           0.0,  0.0, 2000),  # final stop, 2s
]
# Total: ~22 seconds
```

Define a second route for testing:

```python
TEST_ROUTE = [
    Waypoint("check-forward",  0.0,  0.4, 1000),
    Waypoint("check-right",   20.0,  0.4, 1000),
    Waypoint("check-left",   -20.0,  0.4, 1000),
    Waypoint("stop",           0.0,  0.0, 500),
]
```

Routes are plain Python lists — no config files, no JSON. Adding or modifying
a route is a code edit, which is intentional: route changes should be explicit
and version-controlled.

---

## 6. Autonomy node (`autonomy/main.py`)

### 6.1 Configuration

```python
import os

BROKER_HOST  = os.environ.get('MQTT_BROKER_HOST', '172.20.10.2')
BROKER_PORT  = int(os.environ.get('MQTT_BROKER_PORT', '1883'))
CLIENT_ID    = 'less-autonomy'
PUBLISH_HZ   = 20          # commands per second
PUBLISH_MS   = 1000 / PUBLISH_HZ   # 50ms between publishes

TOPIC_CMD    = 'less/v1/cmd/auto'
TOPIC_MODE   = 'less/v1/mode/current'
TOPIC_ESTOP  = 'less/v1/estop'
TOPIC_STATE  = 'less/v1/vcu/state'
TOPIC_STATUS = 'less/v1/vcu/status'
```

### 6.2 MQTT setup

```python
import paho.mqtt.client as mqtt
import json, time, threading

connected    = threading.Event()
vcu_online   = False
mode_current = 'ESTOP'
estop_active = True

def on_connect(client, userdata, flags, rc, properties=None):
    if rc == 0:
        client.subscribe(TOPIC_MODE)
        client.subscribe(TOPIC_ESTOP)
        client.subscribe(TOPIC_STATUS)
        client.subscribe(TOPIC_STATE)
        connected.set()
        print(f"[autonomy] connected to broker")
    else:
        print(f"[autonomy] connect failed: rc={rc}")

def on_message(client, userdata, msg):
    global vcu_online, mode_current, estop_active
    try:
        payload = json.loads(msg.payload.decode())
        if msg.topic == TOPIC_STATUS:
            vcu_online = (payload == 'online' or payload.get('status') == 'online')
        elif msg.topic == TOPIC_MODE:
            mode_current = payload.get('mode', 'ESTOP')
        elif msg.topic == TOPIC_ESTOP:
            estop_active = payload.get('assert', True)
        elif msg.topic == TOPIC_STATE:
            # Available for future use (e.g. reading link_ms)
            pass
    except Exception as e:
        print(f"[autonomy] parse error on {msg.topic}: {e}")

client = mqtt.Client(
    client_id=CLIENT_ID,
    protocol=mqtt.MQTTv5,
    callback_api_version=mqtt.CallbackAPIVersion.VERSION2,
)
client.on_connect = on_connect
client.on_message = on_message
```

### 6.3 Safety checks

```python
def is_safe_to_run() -> tuple[bool, str]:
    """Return (ok, reason). Must be True before starting any route."""
    if not vcu_online:
        return False, "VCU offline"
    if estop_active:
        return False, "ESTOP latched"
    if mode_current != 'AUTO':
        return False, f"mode is {mode_current}, not AUTO"
    return True, "ok"
```

These three conditions must all be true before publishing any non-zero command.
Check before each waypoint, not just at route start.

### 6.4 Route executor

```python
def publish_cmd(steer: float, throttle: float, seq: int):
    payload = json.dumps({
        'steer':    round(steer, 2),
        'throttle': round(throttle, 2),
        'seq':      seq,
    })
    client.publish(TOPIC_CMD, payload, qos=0, retain=False)

def run_route(route: list, name: str = "route"):
    seq = 0
    print(f"\n[autonomy] starting {name} ({len(route)} waypoints)")

    for wp in route:
        ok, reason = is_safe_to_run()
        if not ok:
            print(f"[autonomy] ABORT before '{wp.name}': {reason}")
            publish_cmd(0.0, 0.0, seq)  # explicit safe command
            return False

        print(f"[autonomy] waypoint: {wp.name} "
              f"(steer={wp.steer_deg:+.1f}° throttle={wp.throttle:+.2f} "
              f"duration={wp.duration_ms}ms)")

        deadline = time.monotonic() + wp.duration_ms / 1000.0
        while time.monotonic() < deadline:
            # Re-check safety on every publish cycle
            ok, reason = is_safe_to_run()
            if not ok:
                print(f"[autonomy] ABORT mid-waypoint '{wp.name}': {reason}")
                publish_cmd(0.0, 0.0, seq)
                return False

            publish_cmd(wp.steer_deg, wp.throttle, seq)
            seq += 1
            time.sleep(PUBLISH_MS / 1000.0)

    # Route complete — explicit stop
    publish_cmd(0.0, 0.0, seq)
    print(f"\n[autonomy] {name} complete")
    return True
```

**The mid-waypoint safety check is non-negotiable.** If the operator switches
mode to MANUAL or taps ESTOP mid-route, the executor must detect it within one
publish cycle (50 ms) and stop. This is what makes the demo credible: human
override is instant and the autonomy node obeys.

### 6.5 Main entry point

```python
import argparse

def main():
    parser = argparse.ArgumentParser(description='LESS autonomy node')
    parser.add_argument('--route', choices=['demo', 'test'], default='demo')
    parser.add_argument('--broker', default=BROKER_HOST)
    parser.add_argument('--loop',   action='store_true',
                        help='repeat the route until interrupted')
    args = parser.parse_args()

    # Select route
    from route import DEMO_ROUTE, TEST_ROUTE
    route = DEMO_ROUTE if args.route == 'demo' else TEST_ROUTE
    route_name = args.route

    # Connect
    client.connect(args.broker, BROKER_PORT, keepalive=10)
    client.loop_start()

    print(f"[autonomy] connecting to {args.broker}:{BROKER_PORT}...")
    if not connected.wait(timeout=10):
        print("[autonomy] ERROR: could not connect to broker")
        return 1

    # Wait for retained state to arrive (mode/estop subscriptions)
    time.sleep(0.5)

    # Pre-flight check
    ok, reason = is_safe_to_run()
    if not ok:
        print(f"\n[autonomy] pre-flight FAIL: {reason}")
        print("\nTo arm for AUTO mode:")
        print("  mosquitto_pub -h", args.broker,
              "-t less/v1/estop -r -m '{\"assert\":false,\"src\":\"operator\"}'")
        print("  mosquitto_pub -h", args.broker,
              "-t less/v1/mode/request -m '{\"mode\":\"AUTO\"}'")
        print("\nOr use the ARM button on the dashboard, then switch to AUTO.")
        client.loop_stop()
        return 1

    # Execute
    try:
        while True:
            success = run_route(route, route_name)
            if not args.loop or not success:
                break
            print("[autonomy] looping in 2s...")
            time.sleep(2)
    except KeyboardInterrupt:
        print("\n[autonomy] interrupted — sending stop command")
        client.publish(TOPIC_CMD, json.dumps({'steer':0.0,'throttle':0.0,'seq':0}))
        time.sleep(0.1)

    client.loop_stop()
    client.disconnect()
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
```

---

## 7. Mode switching for AUTO

The autonomy node does **not** request AUTO mode itself. The operator does it
via the dashboard or `mosquitto_pub`. This is intentional — the node should
not be able to arm itself. It only runs when already authorized.

The arming sequence for AUTO:

```bash
# 1. Clear ESTOP (if latched)
mosquitto_pub -h 172.20.10.2 -t less/v1/estop -r \
  -m '{"assert":false,"src":"operator"}'

# 2. Switch to AUTO mode
mosquitto_pub -h 172.20.10.2 -t less/v1/mode/request \
  -m '{"mode":"AUTO"}'

# 3. Run the autonomy node
cd autonomy && python3 main.py --route demo
```

To take back manual control mid-route (the key demo moment):

```bash
# From another terminal, OR tap MANUAL on the dashboard:
mosquitto_pub -h 172.20.10.2 -t less/v1/mode/request \
  -m '{"mode":"MANUAL"}'
```

The route executor detects this within 50 ms and aborts. The car stops.
The joystick immediately takes over.

---

## 8. Dashboard integration for S7.A

The dashboard already has an AUTO mode button (S5). Two small additions
needed in `dashboard/index.html` to make the demo flow smooth. These are
minor UI changes only — no new MQTT topics, no structural changes.

### 8.1 AUTO mode button arms correctly

Currently the AUTO button just publishes `mode/request AUTO`. For the demo,
make it also clear ESTOP first (same as ARM does for MANUAL):

```javascript
// AUTO button handler — update in dashboard/index.html
function requestAuto() {
    // Clear ESTOP first, then request AUTO
    client.publish('less/v1/estop',
        JSON.stringify({ assert: false, src: 'dashboard' }),
        { retain: true, qos: 1 });
    setTimeout(() => {
        client.publish('less/v1/mode/request',
            JSON.stringify({ mode: 'AUTO' }),
            { qos: 1 });
    }, 100);
}
```

### 8.2 Autonomy status indicator

Add a small status line below the mode buttons showing whether the autonomy
node is publishing. Detect it by watching `less/v1/cmd/auto` — if a message
arrives within the last 500 ms, the node is active:

```javascript
let lastAutoCmd = 0;
// In dispatch():
if (topic === 'less/v1/cmd/auto') {
    lastAutoCmd = Date.now();
    client.subscribe('less/v1/cmd/auto'); // subscribe if not already
}
// In a 500ms setInterval:
const autoActive = (Date.now() - lastAutoCmd) < 500;
document.getElementById('auto-status').textContent =
    autoActive ? '🤖 Autonomy node running' : '○ Autonomy node idle';
```

Subscribe to `less/v1/cmd/auto` in the dashboard's `on_connect` handler
alongside the other topics.

---

## 9. `autonomy/README_autonomy.md`

Must contain:

- What this node is and is not (route executor, not perception)
- Prerequisites: broker running, VCU armed and in AUTO mode
- Install: `pip install -r requirements.txt`
- Run commands: `--route demo`, `--route test`, `--loop`
- The manual override procedure (switch to MANUAL mid-route)
- How to add a new route (edit `route.py`, add to `argparse` choices)
- Expected console output for a successful run

---

## 10. Acceptance tests

Run all tests with Mosquitto running. VCU must be flashed and running for T5+.
Watch `mosquitto_sub -h 172.20.10.2 -t 'less/v1/#' -v` throughout.

### T1 — Install and import

```bash
cd autonomy
pip install -r requirements.txt
python3 -c "import paho.mqtt.client; from route import DEMO_ROUTE, TEST_ROUTE; print('ok')"
```

Expect: prints `ok`.

### T2 — Pre-flight check blocks without arming

```bash
python3 main.py --route test
```

Without arming first.

Expect: node connects to broker, waits 500 ms for retained state, prints
pre-flight FAIL with the reason (`ESTOP latched` or `mode is MANUAL`), prints
the arming instructions, exits cleanly. Does NOT publish any commands.

### T3 — Test route executes

Arm for AUTO:
```bash
mosquitto_pub -h 172.20.10.2 -t less/v1/estop -r \
  -m '{"assert":false,"src":"operator"}'
mosquitto_pub -h 172.20.10.2 -t less/v1/mode/request \
  -m '{"mode":"AUTO"}'
```

Then:
```bash
python3 main.py --route test
```

Expect: four waypoints logged, commands published to `less/v1/cmd/auto` at
20 Hz, final stop command sent, script exits 0. Confirm in mosquitto_sub that
steer and throttle values match `TEST_ROUTE` definitions.

### T4 — Manual override aborts route mid-execution

Start demo route:
```bash
python3 main.py --route demo
```

During the `arc-right` waypoint, publish manual mode from another terminal:
```bash
mosquitto_pub -h 172.20.10.2 -t less/v1/mode/request \
  -m '{"mode":"MANUAL"}'
```

Expect: within ~50 ms, the node logs `ABORT mid-waypoint 'arc-right': mode is
MANUAL`, publishes one final `throttle:0.0` stop command, and exits. Confirm
in mosquitto_sub.

### T5 — Physical: car drives the demo route

With VCU running and motors connected:

Arm for AUTO, run:
```bash
python3 main.py --route demo
```

Expect: motors execute the route — forward spin, differential for arcs (front
wheels show speed difference during arc waypoints), stop, reverse, stop.
Total ~22 seconds.

Report which waypoints are physically visible (e.g. the arc-right is the most
distinct since front-right wheel slows visibly).

### T6 — Physical: manual override stops the car instantly

During T5, mid-route, tap MANUAL on the iPad dashboard or run:
```bash
mosquitto_pub -h 172.20.10.2 -t less/v1/mode/request \
  -m '{"mode":"MANUAL"}'
```

Expect: motors stop within ~50 ms. Autonomy node logs ABORT and exits.
Joystick immediately takes over (or car stays stopped with no commands).

**This is the primary demo moment.** It must be instant and reliable.

### T7 — ESTOP stops the car during autonomous run

During T5, tap ESTOP on the dashboard.

Expect: motors stop within one arbiter tick (~150 ms). Autonomy node logs
ABORT (estop latched) and exits. VCU telemetry shows `estop:true`.

### T8 — Loop mode runs repeatedly

```bash
python3 main.py --route test --loop
```

Expect: test route repeats with 2 s pause between runs until Ctrl+C. On
interrupt, one final stop command published, clean exit.

### T9 — Dashboard shows autonomy node active

With dashboard open on iPad and autonomy node running:

Expect: the autonomy status indicator shows `🤖 Autonomy node running`.
When the node finishes or is killed, it changes to `○ Autonomy node idle`
within 500 ms.

---

## 11. Definition of done

- [ ] T1–T9 pass; T5 and T6 reported with physical observations.
- [ ] T6 (manual override) is instant and reliable — test it three times.
- [ ] Pre-flight check prevents running without authorization (T2).
- [ ] Mid-waypoint safety check aborts within 50 ms of mode change (T4).
- [ ] `README_autonomy.md` complete with all sections from §9.
- [ ] Dashboard AUTO button clears ESTOP before requesting AUTO (§8.1).
- [ ] Autonomy status indicator in dashboard (§8.2).
- [ ] `--loop` mode works and Ctrl+C exits cleanly (T8).
- [ ] No hardcoded broker IP — reads from env or `--broker` arg.
- [ ] Script exits non-zero on pre-flight failure, zero on clean completion.

---

## 12. The demo script (for your reference, not the agent)

This is the sequence to run during the actual CEO demo. Practice it twice
beforehand.

```
1. Start broker:      ./infra/run_broker.sh
2. Flash VCU:         cd vcu && cargo run --release
3. Serve dashboard:   ./dashboard/serve.sh
4. Open iPad:         navigate to http://172.20.10.2:8080, connect
5. Arm manually:      tap ARM on dashboard → car in MANUAL
6. Drive manually:    use physical joystick or touch joystick on iPad
                      → show human control works
7. Switch to AUTO:    tap AUTO on dashboard
8. Run autonomy:      python3 autonomy/main.py --route demo
                      → car drives itself through the route
9. Override:          tap MANUAL on dashboard mid-route
                      → car stops instantly, human takes back control
10. ESTOP demo:       run again, tap ESTOP mid-route
                      → car stops, stays stopped until cleared
11. Explain:          "same interface, any perception source —
                      CARLA, camera, LIDAR — just publishes commands here"
```

Step 9 is the moment that lands. Practice the timing so you hit it during
an arc waypoint when the differential is visible.

---

## 13. Reporting back

1. **Test results** — T1–T9. T5 and T6 must include physical description.
2. **T6 timing** — measured delay between mode switch and motor stop.
3. **Files created/modified.**
4. **Deviations.**
5. **Blockers.**
6. **Open questions for S7.B (CARLA)** — if the developer confirms CARLA
   Python client works, what is the minimal bridge between CARLA's waypoint
   follower output and the `less/v1/cmd/auto` topic?