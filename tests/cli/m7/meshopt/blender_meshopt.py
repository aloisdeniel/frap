# SPDX-License-Identifier: GPL-3.0-or-later
#
# WP 7.1: meshopt fixtures and import dumps, run INSIDE Blender 5.2.4:
#
#   $BLENDER_BIN -b --factory-startup --python tests/cli/m7/meshopt/blender_meshopt.py -- make <dir>
#       builds the fixture scene, exports it with export_meshopt_compression_enable=True as
#       ext.glb, khr.glb, ext_sep.gltf (+ .bin) and khr_sep.gltf (+ .bin), the GPU-instancing
#       scene as instances.glb (KHR), and writes <dir>/blender_import.json: Blender's own import
#       of each file (dump below).
#   $BLENDER_BIN -b --factory-startup --python tests/cli/m7/meshopt/blender_meshopt.py -- anim <dir>
#       builds the animated scene (keyed object TRS, a keyed shape key, keyed pose bones) and
#       exports it with meshopt as anim_ext.glb (EXT, the sampled paths: object rotations
#       QUATERNION at byteStride 16, bone rotations at 8, TIME/location/scale EXPONENTIAL,
#       weights SK_ANIM), anim_khr_sep.gltf (+ .bin; KHR, GLTF_SEPARATE, the same paths) and
#       anim_curves.glb (EXT, export_force_sampling=False: the fcurves path, rotations at 8;
#       without the shape-key animation, on which the add-on's fcurves path raises
#       UnboundLocalError in Blender 5.2.4). The golden gltf/meshopt/import_blender_anim reads
#       them from tests/golden/cases/gltf/meshopt/import_blender_anim.inputs/.
#   $BLENDER_BIN -b --factory-startup --python tests/cli/m7/meshopt/blender_meshopt.py -- dump <out.json> <file>...
#       imports each file into an empty scene and writes the same dump (used by the live test
#       `frap-io-gltf` `blender_decodes_frap_compressed_files` on frap-written files).
#
# The committed fixtures live in tests/golden/cases/gltf/meshopt/import_blender.inputs/ (the
# golden `gltf/meshopt/import_blender` and frap-io-gltf's unit tests read them); `make` with that
# directory regenerates them.

import bpy
import json
import os
import sys

argv = sys.argv[sys.argv.index("--") + 1:]


def r(v, n=6):
    return [round(float(c), n) + 0.0 for c in v]


def dump_state():
    objs = {}
    for o in sorted(bpy.data.objects, key=lambda o: o.name):
        d = {
            "type": o.type,
            "parent": o.parent.name if o.parent else None,
            "location": r(o.location),
            "rotation_quaternion": r(o.rotation_quaternion),
            "scale": r(o.scale),
        }
        if o.type == "MESH":
            me = o.data
            d["mesh"] = me.name
            co = []
            for v in me.vertices:
                co += r(v.co)
            d["co"] = co
            d["corner_verts"] = [lp.vertex_index for lp in me.loops]
            d["face_sizes"] = [p.loop_total for p in me.polygons]
            d["edges"] = [list(e.vertices) for e in me.edges]
            uv = me.uv_layers.active
            d["uv"] = [c for l in uv.data for c in r(l.uv)] if uv else []
            if me.shape_keys:
                d["shape_keys"] = {
                    k.name: [c for p in k.data for c in r(p.co)] for k in me.shape_keys.key_blocks
                }
            if o.vertex_groups:
                names = [g.name for g in o.vertex_groups]
                d["weights"] = [
                    sorted([names[g.group], round(g.weight, 5)] for g in v.groups) for v in me.vertices
                ]
        objs[o.name] = d
    return objs


def import_dump(path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    res = bpy.ops.import_scene.gltf(filepath=path)
    return {"result": sorted(res), "objects": dump_state()}


def write_dumps(out_json, named_paths):
    """One compact line per file, keyed by file name, plus the Blender version."""
    dumps = [(name, import_dump(path)) for name, path in named_paths]
    with open(out_json, "w") as f:
        f.write(json.dumps({"blender": bpy.app.version_string}, sort_keys=True)[:-1])
        for name, d in dumps:
            f.write(',\n"%s":%s' % (name, json.dumps(d, sort_keys=True, separators=(",", ":"))))
        f.write("}\n")


def build_scene():
    bpy.ops.wm.read_factory_settings(use_empty=False)
    cube = bpy.data.objects["Cube"]
    bpy.context.view_layer.objects.active = cube
    cube.shape_key_add(name="Basis", from_mix=False)
    k = cube.shape_key_add(name="Lift", from_mix=False)
    for i in range(4):
        k.data[i].co.z += 0.5
    k.value = 0.25
    bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(3.0, 0.0, 0.0))
    bpy.data.objects["Sphere"].rotation_euler = (0.3, 0.0, 0.7)
    me = bpy.data.meshes.new("Edges")
    me.from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 2, 2), (3, 2, 1)],
                   [(4, 5), (5, 0)], [(0, 1, 2, 3)])
    ob = bpy.data.objects.new("Edges", me)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = (0.0, -3.0, 0.0)
    bpy.ops.object.armature_add(location=(0.0, 3.0, 0.0))
    arm = bpy.context.object
    arm.name = "Rig"
    bpy.ops.mesh.primitive_cylinder_add(vertices=8, depth=2.0, radius=0.3, location=(0.0, 3.0, 1.0))
    cyl = bpy.context.object
    cyl.name = "Skinned"
    vg = cyl.vertex_groups.new(name="Bone")
    for v in cyl.data.vertices:
        vg.add([v.index], 0.25 + 0.5 * (v.co.z + 1.0) / 2.0, 'REPLACE')
    mod = cyl.modifiers.new("Armature", 'ARMATURE')
    mod.object = arm
    cyl.parent = arm


def build_instances():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.object.empty_add(location=(0.0, 0.0, 0.0))
    holder = bpy.context.object
    holder.name = "Holder"
    bpy.ops.mesh.primitive_cone_add(vertices=6, location=(0.0, 0.0, 0.0))
    cone = bpy.context.object
    for i, (loc, rot, s) in enumerate([((1.0, 0.0, 0.0), (0.0, 0.0, 0.5), 1.0),
                                       ((0.0, 2.0, 0.0), (0.4, 0.0, 0.0), 0.5),
                                       ((-1.5, 0.0, 1.0), (0.0, 1.2, 0.3), 2.0)]):
        ob = cone if i == 0 else bpy.data.objects.new("Cone.%03d" % i, cone.data)
        if i:
            bpy.context.scene.collection.objects.link(ob)
        ob.location = loc
        ob.rotation_euler = rot
        ob.scale = (s, s, s)
        ob.parent = holder


def build_anim(sk_anim=True):
    bpy.ops.wm.read_factory_settings(use_empty=False)
    cube = bpy.data.objects["Cube"]
    cube.shape_key_add(name="Basis", from_mix=False)
    k = cube.shape_key_add(name="Lift", from_mix=False)
    for i in range(4):
        k.data[i].co.z += 0.5
    if sk_anim:
        k.value = 0.0
        k.keyframe_insert("value", frame=1)
        k.value = 1.0
        k.keyframe_insert("value", frame=10)
    for p in ("location", "rotation_euler", "scale"):
        cube.keyframe_insert(p, frame=1)
    cube.location = (1.0, 2.0, 0.5)
    cube.rotation_euler = (0.3, 0.5, 1.2)
    cube.scale = (1.5, 1.0, 0.5)
    for p in ("location", "rotation_euler", "scale"):
        cube.keyframe_insert(p, frame=10)
    bpy.ops.object.armature_add(location=(0.0, 3.0, 0.0))
    arm = bpy.context.object
    arm.name = "Rig"
    pb = arm.pose.bones["Bone"]
    pb.keyframe_insert("rotation_quaternion", frame=1)
    pb.keyframe_insert("location", frame=1)
    pb.rotation_quaternion = (0.9, 0.3, 0.1, 0.2)
    pb.location = (0.0, 0.5, 0.0)
    pb.keyframe_insert("rotation_quaternion", frame=10)
    pb.keyframe_insert("location", frame=10)


def export(path, ext, fmt, **kw):
    bpy.ops.export_scene.gltf(filepath=path, export_format=fmt, export_meshopt_compression_enable=True,
                              export_meshopt_extension=ext, **kw)


if argv[0] == "make":
    out = os.path.abspath(argv[1])
    os.makedirs(out, exist_ok=True)
    files = []
    for name, ext, fmt in [("ext.glb", "EXT_meshopt_compression", "GLB"),
                           ("khr.glb", "KHR_meshopt_compression", "GLB"),
                           ("ext_sep.gltf", "EXT_meshopt_compression", "GLTF_SEPARATE"),
                           ("khr_sep.gltf", "KHR_meshopt_compression", "GLTF_SEPARATE")]:
        build_scene()
        export(os.path.join(out, name), ext, fmt, export_tangents=True, use_mesh_edges=True)
        files.append(name)
    build_instances()
    export(os.path.join(out, "instances.glb"), "KHR_meshopt_compression", "GLB", export_gpu_instances=True)
    # One dump per file: the sphere's triangle order differs between Blender's exports of the
    # same scene with export_tangents (verified on 5.2.4), so the four imports are not equal.
    write_dumps(os.path.join(out, "blender_import.json"),
                [(name, os.path.join(out, name)) for name in files])
elif argv[0] == "anim":
    out = os.path.abspath(argv[1])
    os.makedirs(out, exist_ok=True)
    build_anim()
    export(os.path.join(out, "anim_ext.glb"), "EXT_meshopt_compression", "GLB")
    build_anim()
    export(os.path.join(out, "anim_khr_sep.gltf"), "KHR_meshopt_compression", "GLTF_SEPARATE")
    build_anim(sk_anim=False)
    export(os.path.join(out, "anim_curves.glb"), "EXT_meshopt_compression", "GLB",
           export_force_sampling=False)
elif argv[0] == "dump":
    write_dumps(argv[1], [(os.path.basename(p), p) for p in argv[2:]])
else:
    raise SystemExit("usage: make <dir> | anim <dir> | dump <out.json> <file>...")
