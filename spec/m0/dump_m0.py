# SPDX-License-Identifier: GPL-3.0-or-later
#
# M0 spec dumper (WP 0.1). Runs INSIDE Blender 5.2:
#
#   $BLENDER_BIN -b --factory-startup --python spec/m0/dump_m0.py -- <out_dir>
#
# Writes the hand-checked M0 facts that the walking skeleton is built from:
#   ops/mesh.primitive_cube_add.json   full operator spec (all props)
#   ops/export_scene.gltf.json         the subset of props frap implements in M0
#   rna/Object.json                    Object.location (+ ID.name, used for coll['Name'])
#   startup.json                       the factory objects/meshes/materials frap builds at start
#   primitives/primitive_cube_add__*.json  reference meshes produced by the operator
#
# The committed copies under spec/m0/ are the source of truth for the hand-written Rust
# statics in spec/m0/*.rs (drift tests compare them) and for the cube generator test.
# The live test `m0_spec_matches_live_blender` (frap-cli) re-runs this script and diffs.
# M1 replaces this with tools/blender/dump_*.py and generated crates (ARCHITECTURE §4).

import bpy
import json
import os
import sys

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = argv[0] if argv else os.path.join(os.path.dirname(__file__))

EXPORT_GLTF_M0_PROPS = ["filepath", "export_format"]


def f32(x):
    return float(x)


def prop_dump(p):
    d = {
        "identifier": p.identifier,
        "name": p.name,
        "description": p.description,
        "type": p.type,
        "subtype": p.subtype,
    }
    if p.type in ("INT", "FLOAT"):
        d["unit"] = p.unit if p.type == "FLOAT" else "NONE"
        d["hard_min"] = p.hard_min
        d["hard_max"] = p.hard_max
        d["soft_min"] = p.soft_min
        d["soft_max"] = p.soft_max
        if p.type == "FLOAT":
            d["precision"] = p.precision
    arr = list(getattr(p, "array_dimensions", []) or [])
    arr = [n for n in arr if n]
    if getattr(p, "array_length", 0) and not arr:
        arr = [p.array_length]
    d["array"] = arr
    if p.type == "ENUM":
        d["items"] = [{"identifier": i.identifier, "name": i.name, "description": i.description}
                      for i in p.enum_items]
        d["default"] = sorted(p.default_flag) if p.is_enum_flag else p.default
        if not d["items"]:
            # Dynamic enum (items from a callback): Blender reports nothing useful in -b.
            # frap's items come from spec/m0/overlay.json (ARCHITECTURE §4.1 dynamic enums).
            d["dynamic"] = True
    elif p.type in ("BOOLEAN", "INT", "FLOAT"):
        d["default"] = [f32(v) if p.type == "FLOAT" else v for v in p.default_array] if arr else p.default
    elif p.type == "STRING":
        d["default"] = p.default
        d["max_bytes"] = p.length_max
    flags = []
    for attr, name in (("is_readonly", "READONLY"), ("is_hidden", "HIDDEN"), ("is_skip_save", "SKIP_SAVE"),
                       ("is_enum_flag", "ENUM_FLAG"), ("is_required", "REQUIRED"), ("is_output", "OUTPUT"),
                       ("is_animatable", "ANIMATABLE"), ("is_never_none", "NEVER_NULL")):
        if getattr(p, attr, False):
            flags.append(name)
    d["flags"] = flags
    return d


def op_dump(op, only=None):
    rna = op.get_rna_type()
    props = [prop_dump(p) for p in rna.properties if p.identifier != "rna_type"
             and (only is None or p.identifier in only)]
    if only is not None:
        props.sort(key=lambda d: only.index(d["identifier"]))
    return {
        "idname": op.idname_py(),
        "bl_idname": op.idname(),
        "label": rna.name,
        "description": rna.description,
        "options": sorted(op.bl_options),
        "props_subset": only is not None,
        "props": props,
    }


def struct_dump(srna, names):
    return {
        "identifier": srna.identifier,
        "base": srna.base.identifier if srna.base else None,
        "name": srna.name,
        "description": srna.description,
        "name_property": srna.name_property.identifier if srna.name_property else None,
        "props": [prop_dump(srna.properties[n]) for n in names],
    }


def mesh_dump(me):
    return {
        "name": me.name,
        "positions": [[f32(c) for c in v.co] for v in me.vertices],
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
        "uv_layers": {l.name: [[f32(c) for c in d.vector] for d in l.uv] for l in me.uv_layers},
        "uv_active": me.uv_layers.active.name if me.uv_layers.active else None,
        "sharp_face": [bool(a.value) for a in me.attributes["sharp_face"].data]
        if "sharp_face" in me.attributes else None,
        "materials": [m.name if m else None for m in me.materials],
        "normals_domain": me.normals_domain,
    }


def obj_dump(o):
    return {
        "name": o.name,
        "type": o.type,
        "data": o.data.name if o.data else None,
        "location": [f32(c) for c in o.location],
        "rotation_mode": o.rotation_mode,
        "rotation_euler": [f32(c) for c in o.rotation_euler],
        "scale": [f32(c) for c in o.scale],
    }


def write(rel, data):
    path = os.path.join(OUT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        json.dump(data, fh, indent=1, sort_keys=False)
        fh.write("\n")


def main():
    stamp = {"blender": bpy.app.version_string, "build_hash": bpy.app.build_hash.decode()}

    write("ops/mesh.primitive_cube_add.json", dict(op_dump(bpy.ops.mesh.primitive_cube_add), _stamp=stamp))
    write("ops/export_scene.gltf.json",
          dict(op_dump(bpy.ops.export_scene.gltf, EXPORT_GLTF_M0_PROPS), _stamp=stamp))
    obj = struct_dump(bpy.types.Object.bl_rna, ["location"])
    id_ = struct_dump(bpy.types.ID.bl_rna, ["name"])
    write("rna/Object.json", dict(obj, _stamp=stamp))
    write("rna/ID.json", dict(id_, _stamp=stamp))
    # Struct headers only (no props bound in M0): the datablock types the skeleton stores.
    # M1 W0 added the remaining ID types of Main (ARCHITECTURE §5.2) as headers.
    for t in ("Mesh", "Camera", "Light", "Material", "Scene", "Collection", "World", "Image",
              "Armature", "Action", "Key", "NodeTree", "Texture", "Library", "PointCloud"):
        write("rna/%s.json" % t, dict(struct_dump(getattr(bpy.types, t).bl_rna, []), _stamp=stamp))

    # Factory startup (before anything is added). "Dots Stroke" is a grease-pencil material
    # that frap omits (ARCHITECTURE §5.7); it is recorded here but skipped by frap.
    scene = bpy.context.scene
    vl = bpy.context.view_layer
    mat = bpy.data.materials["Material"]
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    startup = {
        "objects": [obj_dump(o) for o in bpy.data.objects],
        "meshes": [mesh_dump(m) for m in bpy.data.meshes],
        "cameras": [c.name for c in bpy.data.cameras],
        "lights": [{"name": l.name, "type": l.type, "energy": f32(l.energy)} for l in bpy.data.lights],
        "materials": [{
            "name": mat.name,
            "base_color": [f32(c) for c in bsdf.inputs["Base Color"].default_value],
            "metallic": f32(bsdf.inputs["Metallic"].default_value),
            "roughness": f32(bsdf.inputs["Roughness"].default_value),
            "use_backface_culling": mat.use_backface_culling,
        }],
        "all_material_names": [m.name for m in bpy.data.materials],
        "scene": {
            "name": scene.name,
            "frame_current": scene.frame_current,
            "cursor_location": [f32(c) for c in scene.cursor.location],
            "view_layer": vl.name,
            "base_order": [o.name for o in vl.objects],
            "active_object": vl.objects.active.name if vl.objects.active else None,
            "selected_objects": [o.name for o in bpy.context.selected_objects],
            "master_collection_children": [c.name for c in scene.collection.children],
        },
        "collections": [{"name": c.name, "objects": [o.name for o in c.objects]} for c in bpy.data.collections],
        "_stamp": stamp,
    }
    write("startup.json", startup)

    # Reference meshes from the operator itself (never from the startup cube: different order).
    variants = {
        "default": {},
        "size2_loc001": {"size": 2, "location": (0, 0, 1)},
        "size3_nouv": {"size": 3, "calc_uvs": False},
        "size0": {"size": 0},
        "size_negative": {"size": -1},
    }
    for key, kwargs in variants.items():
        bpy.ops.wm.read_factory_settings(use_empty=True)
        bpy.ops.mesh.primitive_cube_add(**kwargs)
        ob = bpy.context.active_object
        write("primitives/primitive_cube_add__%s.json" % key, {
            "op": "mesh.primitive_cube_add",
            "kwargs": {k: list(v) if isinstance(v, tuple) else v for k, v in kwargs.items()},
            "object": obj_dump(ob),
            "mesh": mesh_dump(ob.data),
            "_stamp": stamp,
        })


main()
