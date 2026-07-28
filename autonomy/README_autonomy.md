# LESS Autonomy Node (S7.A)

## What this is -- and is not

This is a **route executor**, not a perception system. It has no sensors, no
camera, no model. It holds a pre-defined list of waypoints (steering angle,
throttle, duration) and publishes them in order to `less/v1/cmd/auto` at
20 Hz. All the intelligence about *whether* the car is allowed to move lives
in the VCU, not here -- this node cannot arm itself into AUTO mode, cannot
override ESTOP, and stops publishing non-zero commands the instant the VCU's
mode or ESTOP state changes out from under it.

The point of S7.A is to prove the architecture works end-to-end -- MQTT in,
VCU arbitration, actuators out, human override always wins -- with the
simplest possible command source. A future perception system (camera, LIDAR,
CARLA) would plug into the exact same `less/v1/cmd/auto` topic and inherit
all of the same safety guarantees for free.

## Prerequisites

- Mosquitto broker running (`./infra/run_broker.sh`).
- VCU flashed and running, connected to the same broker.
- The VCU must be **armed and in AUTO mode** before running a route -- see
  "Arming for AUTO" below. This node will refuse to run otherwise (that's the
  pre-flight check, by design).

## Install

```bash
cd autonomy
pip install -r requirements.txt
```

## Running

```bash
# The full demo route (~22s): forward, arc-right, arc-left, slow, stop, reverse, stop.
python3 main.py --route demo

# A short smoke-test route (~3.5s): forward, right, left, stop.
python3 main.py --route test

# Repeat the route every 2s until Ctrl+C:
python3 main.py --route demo --loop

# Point at a broker other than the default (env var or --broker, not a code edit):
MQTT_BROKER_HOST=192.168.1.50 python3 main.py --route demo
python3 main.py --route demo --broker 192.168.1.50
```

Exit code is `0` on a clean completed run (or a clean Ctrl+C), `1` if the
broker can't be reached or the pre-flight safety check fails.

## Arming for AUTO

The autonomy node deliberately cannot do this itself -- an operator always
has to authorize AUTO mode first, either via the dashboard or directly:

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

Or on the dashboard: tap **AUTO** (it now clears ESTOP first automatically,
same as ARM does for MANUAL).

## Manual override -- taking back control mid-route

This is the key demo moment: a human can always take back control instantly,
from another terminal or the dashboard, while the route is running:

```bash
mosquitto_pub -h 172.20.10.2 -t less/v1/mode/request \
  -m '{"mode":"MANUAL"}'
```

Or tap **MANUAL** on the dashboard. The route executor re-checks VCU status,
ESTOP, and mode on *every* publish cycle (every 50ms), not just between
waypoints -- so it detects the mode change and stops within one cycle,
publishes an explicit `throttle:0.0` stop command, logs the abort reason, and
exits. Tapping ESTOP instead works the same way (checked in the same
`is_safe_to_run()` call).

## Adding a new route

Routes are plain Python, not a config file or JSON -- a route change is a
code edit, intentionally, so it's explicit and version-controlled.

1. In `route.py`, define a new list of `Waypoint(name, steer_deg, throttle,
   duration_ms)` entries, then wrap it in `[validate(wp) for wp in ...]` like
   `DEMO_ROUTE`/`TEST_ROUTE` (this clamps to the same ranges the VCU enforces,
   as a belt-and-suspenders check).
2. Import it in `main.py` alongside `DEMO_ROUTE`/`TEST_ROUTE`.
3. Add its name to the `--route` argparse `choices` list and the
   `route = DEMO_ROUTE if args.route == "demo" else ...` selection.

## Expected console output for a successful run

```
[autonomy] connecting to 172.20.10.2:1883...
[autonomy] connected to broker

[autonomy] starting demo (10 waypoints)
[autonomy] waypoint: pause (steer=+0.0 throttle=+0.00 duration=2000ms)
[autonomy] waypoint: forward (steer=+0.0 throttle=+0.60 duration=3000ms)
[autonomy] waypoint: arc-right (steer=+25.0 throttle=+0.50 duration=2500ms)
[autonomy] waypoint: straighten (steer=+0.0 throttle=+0.50 duration=1500ms)
[autonomy] waypoint: arc-left (steer=-25.0 throttle=+0.50 duration=2500ms)
[autonomy] waypoint: straighten (steer=+0.0 throttle=+0.50 duration=1500ms)
[autonomy] waypoint: slow (steer=+0.0 throttle=+0.30 duration=1500ms)
[autonomy] waypoint: stop (steer=+0.0 throttle=+0.00 duration=2000ms)
[autonomy] waypoint: reverse (steer=+0.0 throttle=-0.40 duration=2000ms)
[autonomy] waypoint: stop (steer=+0.0 throttle=+0.00 duration=2000ms)

[autonomy] demo complete
```

And on a manual override mid-route:

```
[autonomy] waypoint: arc-right (steer=+25.0 throttle=+0.50 duration=2500ms)
[autonomy] ABORT mid-waypoint 'arc-right': mode is MANUAL, not AUTO
```
