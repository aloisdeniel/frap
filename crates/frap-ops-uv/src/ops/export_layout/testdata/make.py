# SPDX-License-Identifier: GPL-3.0-or-later
# Regenerates the reference files of `export_layout/tests.rs` with Blender 5.2.4:
#   blender -b --factory-startup --python make.py -- <this directory>
# Every call of the add-on's SVG / EPS writers is recorded (its arguments in calls.json, its
# output file next to it), so the unit test can feed frap's writers the same polygon data.
import json
import os
import sys

import bpy

out_dir = sys.argv[sys.argv.index("--") + 1]

import io_mesh_uv_layout.export_uv_eps as eps
import io_mesh_uv_layout.export_uv_svg as svg

calls = []


def recorder(module, kind):
    original = module.export

    def export(filepath, tile, face_data, colors, width, height, opacity):
        calls.append({
            "kind": kind,
            "file": os.path.basename(filepath),
            "tile": list(tile),
            "face_data": [[[list(uv) for uv in uvs], list(color)] for uvs, color in face_data],
            "width": width,
            "height": height,
            "opacity": opacity,
        })
        original(filepath, tile, face_data, colors, width, height, opacity)
    module.export = export


recorder(svg, "SVG")
recorder(eps, "EPS")

# Suzanne with three materials, a face on an empty slot and one past the last slot.
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.mesh.primitive_monkey_add()
ob = bpy.context.object
me = ob.data
for i, rgb in enumerate([(1.0, 0.0, 0.0), (0.2, 0.6, 0.35), (0.0, 0.0, 1.0)]):
    m = bpy.data.materials.new(f"M{i}")
    m.diffuse_color = (*rgb, 1.0)
    me.materials.append(m)
me.materials.append(None)
for p in me.polygons:
    p.material_index = p.index % 5
    p.select = p.index % 3 == 0
os.chdir(out_dir)
bpy.ops.uv.export_layout(filepath="suzanne_all.svg", mode='SVG', export_all=True, size=(300, 200), opacity=0.3)
bpy.ops.uv.export_layout(filepath="suzanne_all.eps", mode='EPS', export_all=True, size=(300, 200), opacity=0.3)
bpy.ops.uv.export_layout(filepath="suzanne_sel.svg", mode='SVG', size=(64, 128), opacity=1.0)
bpy.ops.uv.export_layout(filepath="suzanne_sel.eps", mode='EPS', size=(64, 128), opacity=0.0)

# A cube with UVs over two UDIM tiles (and one exactly on a tile border).
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.mesh.primitive_cube_add()
uv = bpy.context.object.data.uv_layers.active.data
for i, d in enumerate(uv):
    if i >= 12:
        d.uv = (d.uv[0] + 1.0, d.uv[1])
uv[0].uv = (1.0, 1.0)
bpy.ops.uv.export_layout(filepath="cube.1001.svg", mode='SVG', export_all=True, export_tiles='UDIM', size=(32, 32))
bpy.ops.uv.export_layout(filepath="cube.eps", mode='EPS', export_all=True, export_tiles='UV', size=(32, 32), opacity=0.5)

with open(os.path.join(out_dir, "calls.json"), "w") as f:
    json.dump(calls, f, separators=(",", ":"))
