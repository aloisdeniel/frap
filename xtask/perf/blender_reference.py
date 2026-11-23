# SPDX-License-Identifier: GPL-3.0-or-later
#
# Blender's side of the §21.7 performance scenes (M8 WP 8.8), run by `cargo xtask perf blender`:
#
#   blender -b --factory-startup --python xtask/perf/blender_reference.py -- <out.jsonl>
#
# For each scene of `crates/frap-perf-tests` it prints (and appends to <out.jsonl>) one JSON line
# `{"name", "ms", "counts"}`: Blender's time for the same statement and the element counts the
# tests compare against (the "goldens" of the budget tests). Blender's times are a reference
# point, not a budget: `xtask/perf.toml` [blender_reference_ms] keeps the values measured on the
# machine named there.

import json
import os
import struct
import sys
import tempfile
import time

import bpy

OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else None


def emit(name, ms, **extra):
    line = json.dumps({"name": name, "ms": round(ms, 3), **extra}, sort_keys=True)
    print("BLENDER_PERF " + line)
    if OUT:
        with open(OUT, "a") as f:
            f.write(line + "\n")


def edit_scene(big):
    """The edit-op scene: a big grid in Edit Mode plus a 12 x 12 grid added in Edit Mode."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=big, y_subdivisions=big, size=2.0)
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.primitive_grid_add(
        x_subdivisions=12, y_subdivisions=12, size=0.5, location=(0, 0, 0.5))
    return bpy.context.object


def counts(ob):
    ob.update_from_editmode()
    me = ob.data
    return {
        "elements": [len(me.vertices), len(me.edges), len(me.polygons)],
        "selected": [me.total_vert_sel, me.total_edge_sel, me.total_face_sel],
    }


EDIT_OPS = {
    "subdivide": lambda: bpy.ops.mesh.subdivide(),
    "extrude_region": lambda: bpy.ops.mesh.extrude_region(),
    "bevel": lambda: bpy.ops.mesh.bevel(offset=0.01),
}

ob = edit_scene(316)
emit("edit_op_100k.scene", 0.0, counts=counts(ob))
for name, op in EDIT_OPS.items():
    times = []
    for rep in range(4):
        ob = edit_scene(316)
        t = time.perf_counter()
        op()
        times.append((time.perf_counter() - t) * 1e3)
    emit("edit_op_100k." + name, sorted(times[1:])[1], counts=counts(ob))

# Whole-mesh variants (everything selected): what a 100k-face operator costs Blender when it
# rewrites the whole mesh. Not a frap budget.
for name, op in [("subdivide", EDIT_OPS["subdivide"]), ("extrude_region", EDIT_OPS["extrude_region"])]:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=317, y_subdivisions=317)
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    t = time.perf_counter()
    op()
    emit("edit_op_100k_all." + name, (time.perf_counter() - t) * 1e3,
         counts=counts(bpy.context.object))

# GLB export of 1M corners.
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=500, y_subdivisions=500, size=2.0)
path = os.path.join(tempfile.mkdtemp(prefix="frap-perf-"), "grid.glb")
times = []
for rep in range(4):
    t = time.perf_counter()
    bpy.ops.export_scene.gltf(filepath=path, export_format='GLB')
    times.append((time.perf_counter() - t) * 1e3)
data = open(path, "rb").read()
json_len = struct.unpack("<I", data[12:16])[0]
off = 20 + json_len
bin_len = struct.unpack("<I", data[off:off + 4])[0]
h = 0xCBF29CE484222325
for b in data[off + 8:off + 8 + bin_len]:
    h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
emit("glb_export_1m_corners", sorted(times[1:])[1],
     bytes=len(data), bin_len=bin_len, bin_fnv1a=hex(h),
     json=json.loads(data[20:20 + json_len]))
os.remove(path)
