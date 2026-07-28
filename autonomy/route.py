"""Waypoint route definitions for the LESS autonomy node.

A route is a plain Python list of Waypoints, executed in order. This node is
a route executor, not a perception system -- there are no sensors, no model,
no camera. Routes are code, not config, so a route change is an explicit,
version-controlled edit.
"""

from typing import NamedTuple


class Waypoint(NamedTuple):
    name: str  # human-readable label, logged on entry
    steer_deg: float  # degrees, clamped to [-35, 35]
    throttle: float  # [-1.0, 1.0], positive = forward
    duration_ms: int  # how long to hold this waypoint


def _clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def validate(wp: Waypoint) -> Waypoint:
    """Mirrors shared::DriveCommand::clamped() -- belt-and-suspenders on top
    of the VCU's own clamping, so a typo'd route can't produce an
    out-of-range command even transiently."""
    return wp._replace(
        steer_deg=_clamp(wp.steer_deg, -35.0, 35.0),
        throttle=_clamp(wp.throttle, -1.0, 1.0),
    )


# Bench rig: wheels free-spinning, not driving on a surface. The route is
# designed to be visible in wheel speed/direction, not physical displacement.
# ~22 seconds total -- long enough to demonstrate, short enough to repeat
# twice during a demo.
DEMO_ROUTE = [
    Waypoint("pause", 0.0, 0.0, 2000),  # 2s armed, not moving
    Waypoint("forward", 0.0, 0.6, 3000),  # straight ahead, 3s
    Waypoint("arc-right", 25.0, 0.5, 2500),  # right arc, 2.5s
    Waypoint("straighten", 0.0, 0.5, 1500),  # straighten, 1.5s
    Waypoint("arc-left", -25.0, 0.5, 2500),  # left arc, 2.5s
    Waypoint("straighten", 0.0, 0.5, 1500),  # straighten, 1.5s
    Waypoint("slow", 0.0, 0.3, 1500),  # decelerate, 1.5s
    Waypoint("stop", 0.0, 0.0, 2000),  # full stop, 2s
    Waypoint("reverse", 0.0, -0.4, 2000),  # reverse, 2s
    Waypoint("stop", 0.0, 0.0, 2000),  # final stop, 2s
]

# Short route for T2/T3-style smoke tests: forward, right, left, stop.
TEST_ROUTE = [
    Waypoint("check-forward", 0.0, 0.4, 1000),
    Waypoint("check-right", 20.0, 0.4, 1000),
    Waypoint("check-left", -20.0, 0.4, 1000),
    Waypoint("stop", 0.0, 0.0, 500),
]

DEMO_ROUTE = [validate(wp) for wp in DEMO_ROUTE]
TEST_ROUTE = [validate(wp) for wp in TEST_ROUTE]
