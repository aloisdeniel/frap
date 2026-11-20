# SPDX-License-Identifier: GPL-3.0-or-later
# Regenerates the Blender fixtures of `tests.rs` (WP 6.7): imports Khronos samples with
# Blender 5.2.4's add-on into an empty file and dumps every action (slots, F-curves, keys)
# and every object's animation data (assigned action and slot, NLA tracks and strips).
#
#   Blender -b --factory-startup --python dump_blender_import.py -- <repo root>
import json
import os
import sys

import bpy

ROOT = sys.argv[sys.argv.index("--") + 1]
SAMPLES = [
    "InterpolationTest/glTF-Binary/InterpolationTest.glb",
    "AnimatedTriangle/glTF-Embedded/AnimatedTriangle.gltf",
]


def f(v):
    return [float(x) for x in v]


def dump():
    actions = {}
    for a in bpy.data.actions:
        curves = []
        for layer in a.layers:
            for strip in layer.strips:
                for bag in strip.channelbags:
                    for fc in bag.fcurves:
                        curves.append({
                            "slot": bag.slot.identifier,
                            "data_path": fc.data_path,
                            "array_index": fc.array_index,
                            "group": fc.group.name if fc.group else None,
                            "keys": [[f(k.co), k.interpolation, f(k.handle_left), f(k.handle_right),
                                      k.handle_left_type, k.handle_right_type] for k in fc.keyframe_points],
                        })
        actions[a.name] = {"slots": [s.identifier for s in a.slots], "fcurves": curves,
                           "frame_range": f(a.frame_range)}
    objects = {}
    for o in bpy.data.objects:
        ad = o.animation_data
        if ad is None:
            continue
        objects[o.name] = {
            "action": ad.action.name if ad.action else None,
            "slot": ad.action_slot.identifier if ad.action_slot else None,
            "tracks": [{
                "name": t.name, "mute": t.mute, "lock": t.lock,
                "strips": [{"name": s.name, "action": s.action.name, "slot": s.action_slot.identifier,
                            "frame_start": s.frame_start, "frame_end": s.frame_end,
                            "action_frame_start": s.action_frame_start,
                            "action_frame_end": s.action_frame_end} for s in t.strips],
            } for t in ad.nla_tracks],
        }
    return {"actions": actions, "objects": objects}


out = {}
for rel in SAMPLES:
    bpy.ops.wm.read_homefile(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.join(ROOT, "tests/data/gltf-samples", rel))
    out[rel] = dump()
path = os.path.join(ROOT, "crates/frap-io-gltf/src/import/animation/blender_import.json")
with open(path, "w") as fh:
    json.dump(out, fh, indent=1, sort_keys=True)
print("wrote", path, bpy.app.version_string)
