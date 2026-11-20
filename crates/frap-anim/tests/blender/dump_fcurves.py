# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender side of `crates/frap-anim/tests/blender_fcurves.rs` (WP 6.1).

Builds F-curves through the Action data API (`fcurves.new`, `keyframe_points.insert`, key and
curve properties, `FCurve.update()`), then dumps every key (co, handles, types) and the value
sampled every quarter frame from 3 frames before the first key to 3 after the last. The spec
of each curve is written next to the results, so the Rust test builds the same curves with
frap-anim and compares.

Run with Blender 5.2.4 LTS:

    Blender -b --factory-startup --python crates/frap-anim/tests/blender/dump_fcurves.py -- \
        crates/frap-anim/tests/fixtures/blender_fcurves.json
"""

import json
import sys

import bpy

IPOS = ["CONSTANT", "LINEAR", "BEZIER", "SINE", "QUAD", "CUBIC", "QUART", "QUINT", "EXPO",
        "CIRC", "BACK", "BOUNCE", "ELASTIC"]
EASINGS = ["AUTO", "EASE_IN", "EASE_OUT", "EASE_IN_OUT"]
HANDLES = ["FREE", "ALIGNED", "VECTOR", "AUTO", "AUTO_CLAMPED"]
WAVE = [[1, 0.0], [5, 2.0], [9, -1.0], [14, 3.5], [20, 3.5], [26, 0.25]]


def cases():
    out = []
    # Every interpolation with every easing, on a curve that goes up then down.
    for ipo in IPOS:
        for easing in EASINGS:
            out.append({
                "name": f"{ipo}_{easing}",
                "keys": [[1, 0.0], [11, 4.0], [21, -2.0]],
                "key_props": [{"interpolation": ipo, "easing": easing}] * 3,
            })
    # BACK overshoot and ELASTIC amplitude / period.
    out.append({"name": "BACK_overshoot", "keys": [[1, 0.0], [9, 3.0]],
                "key_props": [{"interpolation": "BACK", "easing": "EASE_IN_OUT", "back": 3.0}, {}]})
    for amp, per in [(0.0, 0.0), (5.0, 2.0), (0.2, 6.0)]:
        out.append({"name": f"ELASTIC_{amp}_{per}", "keys": [[1, 0.0], [13, 3.0]],
                    "key_props": [{"interpolation": "ELASTIC", "easing": "EASE_IN_OUT",
                                   "amplitude": amp, "period": per}, {}]})
    # Handle types (both sides), with both smoothing modes and both extrapolations.
    for smoothing in ["NONE", "CONT_ACCEL"]:
        for extrapolation in ["CONSTANT", "LINEAR"]:
            for h in HANDLES:
                props = [{"handle_left_type": h, "handle_right_type": h}] * len(WAVE)
                if h in ("FREE", "ALIGNED"):
                    props = [dict(p, handle_left=[k[0] - 1.5, k[1] - 0.5],
                                  handle_right=[k[0] + 2.0, k[1] + 1.0]) for p, k in zip(props, WAVE)]
                out.append({"name": f"{h}_{smoothing}_{extrapolation}", "keys": WAVE,
                            "auto_smoothing": smoothing, "extrapolation": extrapolation,
                            "key_props": props})
            # Mixed handle types along one curve.
            mixed = [{"handle_left_type": "VECTOR", "handle_right_type": "VECTOR"},
                     {"handle_left_type": "AUTO", "handle_right_type": "AUTO"},
                     {"handle_left_type": "ALIGNED", "handle_right_type": "ALIGNED",
                      "handle_left": [3.0, 0.0], "handle_right": [7.5, 4.0]},
                     {},
                     {"handle_left_type": "FREE", "handle_right_type": "VECTOR",
                      "handle_left": [18.0, 5.0]},
                     {"handle_left_type": "AUTO", "handle_right_type": "AUTO"}]
            out.append({"name": f"mixed_{smoothing}_{extrapolation}", "keys": WAVE,
                        "auto_smoothing": smoothing, "extrapolation": extrapolation,
                        "key_props": mixed})
            out.append({"name": f"single_{smoothing}_{extrapolation}", "keys": [[4, 2.0]],
                        "auto_smoothing": smoothing, "extrapolation": extrapolation})
            out.append({"name": f"linear_ends_{smoothing}_{extrapolation}",
                        "keys": [[1, 1.0], [6, 3.0], [12, 2.0]],
                        "auto_smoothing": smoothing, "extrapolation": extrapolation,
                        "key_props": [{"interpolation": "LINEAR"}, {}, {"interpolation": "LINEAR"}]})
    # Keys inserted out of order, close together (deduplicated by update()), replaced.
    out.append({"name": "unordered_dupes", "keys": [[10, 1.0], [2, 0.0], [6, 5.0], [6.004, 2.0],
                                                     [2, 3.0], [14.5, -1.0]]})
    # A key inserted between manual handles keeps the curve shape (subdivision).
    out.append({"name": "subdivide_free", "keys": [[1, 0.0], [21, 10.0]],
                "key_props": [{"handle_left_type": "FREE", "handle_right_type": "FREE",
                               "handle_right": [9.0, 6.0]},
                              {"handle_left_type": "FREE", "handle_right_type": "FREE",
                               "handle_left": [15.0, 1.0]}],
                "then_insert": [[8, 0.0], [13.5, 0.0]]})
    # Handles past the neighbouring key (corrected segment).
    out.append({"name": "long_handles", "keys": [[1, 0.0], [5, 4.0]],
                "key_props": [{"handle_left_type": "FREE", "handle_right_type": "FREE",
                               "handle_right": [12.0, 9.0]},
                              {"handle_left_type": "FREE", "handle_right_type": "FREE",
                               "handle_left": [-6.0, -3.0]}]})
    # Clamped extremes and flat stretches.
    out.append({"name": "clamped_extremes", "keys": [[1, 0.0], [3, 5.0], [4, 5.0], [9, -2.0],
                                                      [10, 7.0], [30, 6.5], [31, 6.0]]})
    return out


def frames(keys):
    lo = min(k[0] for k in keys) - 3
    hi = max(k[0] for k in keys) + 3
    out = []
    f = lo
    while f <= hi + 1e-9:
        out.append(f)
        f += 0.25
    return out


def build(bag, case, i):
    fc = bag.fcurves.new(f'["prop_{i}"]', index=0)
    if "auto_smoothing" in case:
        fc.auto_smoothing = case["auto_smoothing"]
    if "extrapolation" in case:
        fc.extrapolation = case["extrapolation"]
    for f, v in case["keys"]:
        fc.keyframe_points.insert(f, v)
    for k, p in zip(list(fc.keyframe_points), case.get("key_props", [])):
        for name, value in p.items():
            setattr(k, name, value)
    if case.get("key_props"):
        fc.update()
    for f, v in case.get("then_insert", []):
        fc.keyframe_points.insert(f, v)
    return fc


def main():
    out_path = sys.argv[sys.argv.index("--") + 1]
    act = bpy.data.actions.new("Curves")
    slot = act.slots.new('OBJECT', "Cube")
    strip = act.layers.new("Layer").strips.new(type='KEYFRAME')
    bag = strip.channelbag(slot, ensure=True)
    results = []
    for i, case in enumerate(cases()):
        fc = build(bag, case, i)
        keys = [{
            "co": list(k.co), "handle_left": list(k.handle_left), "handle_right": list(k.handle_right),
            "handle_left_type": k.handle_left_type, "handle_right_type": k.handle_right_type,
            "interpolation": k.interpolation,
        } for k in fc.keyframe_points]
        samples = [[f, fc.evaluate(f)] for f in frames([k["co"] for k in keys])]
        results.append({"case": case, "keys": keys, "samples": samples})
    with open(out_path, "w") as fh:
        json.dump({"blender": bpy.app.version_string, "build_hash": bpy.app.build_hash.decode(),
                   "curves": results}, fh, indent=0)


main()
