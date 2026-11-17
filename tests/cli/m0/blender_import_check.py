# SPDX-License-Identifier: GPL-3.0-or-later
#
# Runs INSIDE Blender 5.2 (used by crates/frap-cli/tests/blender_live.rs):
#
#   $BLENDER_BIN -b --factory-startup --python tests/cli/m0/blender_import_check.py -- <frap.glb> <out.json> <tmpdir>
#
# 1. Runs the M0 exit transcript in Blender itself and exports Blender's own GLB (reference).
# 2. Imports frap's GLB and Blender's GLB, each into an empty scene, with import_scene.gltf.
# 3. Writes a canonical dump of both imports (objects, transforms, meshes as sorted
#    triangles of (position, normal, uv) tuples, materials) so the Rust test can compare.

import bpy
import json
import os
import sys

frap_glb, out_json, tmpdir = sys.argv[sys.argv.index("--") + 1:]


def r(v, n=5):
    return [round(float(c), n) + 0.0 for c in v]


def dump():
    objs = {}
    for o in bpy.data.objects:
        d = {
            "type": o.type,
            "parent": o.parent.name if o.parent else None,
            "location": r(o.location),
            "rotation_quaternion": r(o.rotation_quaternion) if o.rotation_mode == "QUATERNION" else None,
            "rotation_euler": r(o.rotation_euler),
            "scale": r(o.scale),
        }
        if o.type == "MESH":
            me = o.data
            uv = me.uv_layers.active
            me.calc_loop_triangles()
            tris = []
            for t in me.loop_triangles:
                corners = []
                for li in t.loops:
                    loop = me.loops[li]
                    corners.append((
                        tuple(r(me.vertices[loop.vertex_index].co)),
                        tuple(r(me.corner_normals[li].vector, 4)),
                        tuple(r(uv.uv[li].vector)) if uv else None,
                    ))
                # rotation-normalised corner cycle
                k = min(range(3), key=lambda i: corners[i])
                tris.append(corners[k:] + corners[:k])
            d["mesh"] = {
                "name": me.name,
                "verts": len(me.vertices),
                "polys": len(me.polygons),
                "uv_layers": [l.name for l in me.uv_layers],
                "materials": [m.name if m else None for m in me.materials],
                "triangles": sorted(tris),
            }
        objs[o.name] = d
    mats = {}
    for m in bpy.data.materials:
        if m.users == 0 or not m.node_tree:
            continue
        bsdf = m.node_tree.nodes.get("Principled BSDF")
        mats[m.name] = {
            "use_backface_culling": m.use_backface_culling,
            "base_color": r(bsdf.inputs["Base Color"].default_value) if bsdf else None,
            "metallic": round(bsdf.inputs["Metallic"].default_value, 5) if bsdf else None,
            "roughness": round(bsdf.inputs["Roughness"].default_value, 5) if bsdf else None,
        }
    return {"objects": objs, "materials": mats}


# 1. Blender's own result for the transcript.
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
bpy.data.objects["Cube"].location = (1, 2, 3)
ref = os.path.join(tmpdir, "blender_m0.glb")
bpy.ops.export_scene.gltf(filepath=ref)

# 2. Import both into empty scenes.
out = {"blender": bpy.app.version_string, "build_hash": bpy.app.build_hash.decode()}
for key, path in (("frap", frap_glb), ("blender_export", ref)):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    try:
        res = sorted(bpy.ops.import_scene.gltf(filepath=path))
        out[key] = {"result": res, **dump()}
    except Exception as e:  # reported, the Rust test fails with the message
        out[key] = {"result": ["EXCEPTION"], "error": "%s: %s" % (type(e).__name__, e)}

with open(out_json, "w", encoding="utf-8") as fh:
    json.dump(out, fh, indent=1, sort_keys=True)
