# SPDX-License-Identifier: GPL-3.0-or-later
# WP 8.10: the M1 exit transcript (tests/cli/m1/vertical_slice.in/vertical_slice.py, both
# syntaxes written as Python) run by Blender 5.2 itself in the current directory, then the
# state facts that tests/data/dump_state.lua prints from frap's Lua front-end, one per line.
#
#   cd <tmp> && Blender -b --factory-startup --python m1_transcript_state.py -- <out.txt>
#
# The committed reference is tests/data/m1_transcript_blender.txt (Blender 5.2.4).
import os
import sys

import bpy

out_path = sys.argv[sys.argv.index("--") + 1]
os.makedirs("out", exist_ok=True)

bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.5, depth=2, location=(0, 0, 1))
bpy.data.materials.new("Steel")
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Steel'].diffuse_color = (0.6, 0.6, 0.7, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Steel'])
bpy.ops.object.camera_add(location=(4, -4, 3), rotation=(1.1, 0, 0.785))
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.data.scenes['Scene'].render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.filepath = 'out/part.png'
bpy.ops.render.render(write_still=True)
bpy.ops.export_scene.gltf(filepath='out/part.glb')
bpy.ops.wm.save_as_mainfile(filepath='out/part.frap')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/part.glb')
imported_metallic = bpy.data.objects['Cylinder'].data.materials[0].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value
bpy.ops.wm.open_mainfile(filepath='out/part.frap')

lines = []


def num(x):
    return repr(float(x))


def fact(*parts):
    lines.append(" ".join(str(p) for p in parts))


fact("import", "metallic", num(imported_metallic))
fact("render", "png", int(os.path.isfile("out/part.png")))
for ob in sorted(bpy.data.objects, key=lambda o: o.name):
    fact("object", ob.name, "type", ob.type)
    fact("object", ob.name, "data", ob.data.name if ob.data else "None")
    fact("object", ob.name, "location", *map(num, ob.location))
    fact("object", ob.name, "rotation_euler", *map(num, ob.rotation_euler))
    fact("object", ob.name, "scale", *map(num, ob.scale))
for me in sorted(bpy.data.meshes, key=lambda m: m.name):
    fact("mesh", me.name, "vertices", len(me.vertices), "edges", len(me.edges),
         "polygons", len(me.polygons))
    fact("mesh", me.name, "materials", *[m.name if m else "None" for m in me.materials])
for ma in sorted(bpy.data.materials, key=lambda m: m.name):
    fact("material", ma.name, "diffuse_color", *map(num, ma.diffuse_color))
    fact("material", ma.name, "metallic",
         num(ma.node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value))
for ca in sorted(bpy.data.cameras, key=lambda c: c.name):
    fact("camera", ca.name, "lens", num(ca.lens))
sc = bpy.context.scene
fact("scene", sc.name, "camera", sc.camera.name if sc.camera else "None")
fact("scene", sc.name, "engine", sc.render.engine)
fact("scene", sc.name, "filepath", sc.render.filepath)
fact("context", "object", bpy.context.object.name if bpy.context.object else "None")

with open(out_path, "w") as f:
    f.write("\n".join(lines) + "\n")
