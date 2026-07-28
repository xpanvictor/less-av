"""LESS autonomy node: a waypoint route executor.

This is not a perception system. It has no sensors, no camera, no model. It
publishes a pre-defined sequence of DriveCommand messages to
less/v1/cmd/auto and relies entirely on the VCU's own mode arbitration,
deadman timeout, and ESTOP latch for safety -- the same as any other command
source. It cannot arm itself into AUTO mode; an operator (dashboard or
mosquitto_pub) must do that first.

See README_autonomy.md for how to run this and what to expect.
"""

import argparse
import json
import os
import threading
import time

import paho.mqtt.client as mqtt

from route import DEMO_ROUTE, TEST_ROUTE

BROKER_HOST = os.environ.get("MQTT_BROKER_HOST", "172.20.10.2")
BROKER_PORT = int(os.environ.get("MQTT_BROKER_PORT", "1883"))
CLIENT_ID = "less-autonomy"

PUBLISH_HZ = 20  # commands per second
PUBLISH_MS = 1000 / PUBLISH_HZ  # 50ms between publishes

TOPIC_CMD = "less/v1/cmd/auto"
TOPIC_MODE = "less/v1/mode/current"
TOPIC_ESTOP = "less/v1/estop"
TOPIC_STATE = "less/v1/vcu/state"
TOPIC_STATUS = "less/v1/vcu/status"

# --- MQTT state, updated only from on_message (paho's network thread) ------

connected = threading.Event()
vcu_online = False
mode_current = "ESTOP"
estop_active = True


def on_connect(client, userdata, flags, rc, properties=None):
    if rc == 0:
        client.subscribe(TOPIC_MODE)
        client.subscribe(TOPIC_ESTOP)
        client.subscribe(TOPIC_STATUS)
        client.subscribe(TOPIC_STATE)
        connected.set()
        print("[autonomy] connected to broker")
    else:
        print(f"[autonomy] connect failed: rc={rc}")


def on_message(client, userdata, msg):
    global vcu_online, mode_current, estop_active
    try:
        payload = json.loads(msg.payload.decode())
        if msg.topic == TOPIC_STATUS:
            # vcu/status is a bare JSON string: "online" or "offline"
            # (see docs/PROTOCOL.md) -- not an object.
            vcu_online = payload == "online"
        elif msg.topic == TOPIC_MODE:
            mode_current = payload.get("mode", "ESTOP")
        elif msg.topic == TOPIC_ESTOP:
            estop_active = payload.get("assert", True)
        elif msg.topic == TOPIC_STATE:
            # Available for future use (e.g. reading link_ms).
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


# --- Safety --------------------------------------------------------------


def is_safe_to_run() -> tuple[bool, str]:
    """Return (ok, reason). Must be True before publishing any non-zero
    command, and is re-checked before every waypoint AND on every publish
    cycle within a waypoint -- not just once at route start."""
    if not vcu_online:
        return False, "VCU offline"
    if estop_active:
        return False, "ESTOP latched"
    if mode_current != "AUTO":
        return False, f"mode is {mode_current}, not AUTO"
    return True, "ok"


# --- Route execution -------------------------------------------------------


def publish_cmd(steer: float, throttle: float, seq: int) -> None:
    payload = json.dumps(
        {
            "steer": round(steer, 2),
            "throttle": round(throttle, 2),
            "seq": seq,
        }
    )
    client.publish(TOPIC_CMD, payload, qos=0, retain=False)


def run_route(route: list, name: str = "route") -> bool:
    seq = 0
    print(f"\n[autonomy] starting {name} ({len(route)} waypoints)")

    for wp in route:
        ok, reason = is_safe_to_run()
        if not ok:
            print(f"[autonomy] ABORT before '{wp.name}': {reason}")
            publish_cmd(0.0, 0.0, seq)
            return False

        print(
            f"[autonomy] waypoint: {wp.name} "
            f"(steer={wp.steer_deg:+.1f} throttle={wp.throttle:+.2f} "
            f"duration={wp.duration_ms}ms)"
        )

        deadline = time.monotonic() + wp.duration_ms / 1000.0
        while time.monotonic() < deadline:
            # Re-check safety on every publish cycle. This is what makes the
            # human-override demo moment credible: switching to MANUAL or
            # tapping ESTOP is detected within one 50ms publish cycle, not
            # just at the next waypoint boundary.
            ok, reason = is_safe_to_run()
            if not ok:
                print(f"[autonomy] ABORT mid-waypoint '{wp.name}': {reason}")
                publish_cmd(0.0, 0.0, seq)
                return False

            publish_cmd(wp.steer_deg, wp.throttle, seq)
            seq += 1
            time.sleep(PUBLISH_MS / 1000.0)

    publish_cmd(0.0, 0.0, seq)
    print(f"\n[autonomy] {name} complete")
    return True


# --- Entry point -----------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description="LESS autonomy node")
    parser.add_argument("--route", choices=["demo", "test"], default="demo")
    parser.add_argument("--broker", default=BROKER_HOST)
    parser.add_argument(
        "--loop", action="store_true", help="repeat the route until interrupted"
    )
    args = parser.parse_args()

    route = DEMO_ROUTE if args.route == "demo" else TEST_ROUTE
    route_name = args.route

    print(f"[autonomy] connecting to {args.broker}:{BROKER_PORT}...")
    try:
        client.connect(args.broker, BROKER_PORT, keepalive=10)
    except OSError as e:
        # client.connect() is a blocking socket call and raises on a bad
        # host/unreachable broker -- a very plausible failure mode if the
        # venue's broker IP differs from what's baked in, so this must not
        # crash with a raw traceback.
        print(f"[autonomy] ERROR: could not connect to {args.broker}:{BROKER_PORT}: {e}")
        return 1
    client.loop_start()

    if not connected.wait(timeout=10):
        print("[autonomy] ERROR: could not connect to broker")
        client.loop_stop()
        return 1

    # Wait for retained state (mode/current, estop, vcu/status) to arrive.
    time.sleep(0.5)

    ok, reason = is_safe_to_run()
    if not ok:
        print(f"\n[autonomy] pre-flight FAIL: {reason}")
        print("\nTo arm for AUTO mode:")
        print(
            "  mosquitto_pub -h",
            args.broker,
            "-t less/v1/estop -r -m '{\"assert\":false,\"src\":\"operator\"}'",
        )
        print(
            "  mosquitto_pub -h",
            args.broker,
            "-t less/v1/mode/request -m '{\"mode\":\"AUTO\"}'",
        )
        print("\nOr use the ARM button on the dashboard, then switch to AUTO.")
        client.loop_stop()
        client.disconnect()
        return 1

    try:
        while True:
            success = run_route(route, route_name)
            if not args.loop or not success:
                break
            print("[autonomy] looping in 2s...")
            time.sleep(2)
    except KeyboardInterrupt:
        print("\n[autonomy] interrupted -- sending stop command")
        client.publish(TOPIC_CMD, json.dumps({"steer": 0.0, "throttle": 0.0, "seq": 0}))
        time.sleep(0.1)

    client.loop_stop()
    client.disconnect()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
