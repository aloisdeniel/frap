# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump the complete factory-startup state to JSON (ARCHITECTURE.md §4.1, §5.7).

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_startup.py -- --out spec/blender-5.2/

Writes <out>/startup.json:

* `blender`: version, build hash, add-on preferences used (§4.1);
* `ids`: for every non-empty `bpy.data` collection, its IDs in `bpy.data` order, each as a
  generic RNA serialisation: every property (base chain included) except the per-session and
  UI-only ones in SKIP; ID pointers as `{"$ref": repr}`, nested non-ID structs recursively
  (depth-limited), collections of structs as lists, ID collections as lists of refs;
* `meshes_exact`: for every mesh, the exact arrays (positions, edges, polygon loop starts and
  totals, corner verts and edges, every attribute with domain/data_type/data, UV layers): the
  default cube's vertex order differs from `primitive_cube_add`'s;
* `window`: `bpy.context.window` scene / view layer / workspace / screen;
* `view3d`: the Layout workspace's 3D viewport: `RegionView3D` matrices and view settings and
  the `SpaceView3D` lens, clip and shading (the virtual 3D View of §8);
* `context`: mode, active object, selected objects (names).
"""

import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dump_rna  # noqa: E402  (shared JSON helpers)

MAX_DEPTH = 4
MAX_ITEMS = 64
SKIP = {
    "rna_type", "session_uid", "original", "evaluated_get", "id_data", "preview", "bindcode",
    "users_collection", "users_scene", "children", "children_recursive", "library_weak_reference",
    "override_library", "asset_data", "bl_rna", "is_evaluated", "is_runtime_data",
    "persistent_uid", "uuid", "image_pixels", "icon_pixels", "image_pixels_float",
    "icon_pixels_float", "depsgraph", "objects_in_mode",
}
# Structs not descended into (UI/runtime-only or huge).
SKIP_STRUCTS = {"Struct", "BlendData", "Preferences", "Theme", "WindowManager", "Screen",
                "Area", "Region", "Depsgraph", "ImagePreview", "AssetMetaData", "IDOverrideLibrary",
                "SequenceEditor", "RigidBodyWorld"}


def ser(v, p, depth):
    if p.type == "POINTER":
        if v is None:
            return None
        if isinstance(v, bpy.types.ID):
            return {"$ref": repr(v)}
        if depth >= MAX_DEPTH or v.bl_rna.identifier in SKIP_STRUCTS:
            return {"$ptr": repr(v), "type": v.bl_rna.identifier}
        return struct(v, depth + 1)
    if p.type == "COLLECTION":
        items = list(v)
        if p.fixed_type is not None and dump_rna.is_id_struct(p.fixed_type):
            return [dump_rna.jvalue(x) for x in items]
        if depth >= MAX_DEPTH or (p.fixed_type and p.fixed_type.identifier in SKIP_STRUCTS):
            return {"$len": len(items)}
        out = [struct(x, depth + 1) for x in items[:MAX_ITEMS]]
        if len(items) > MAX_ITEMS:
            out.append({"$truncated": len(items)})
        return out
    return dump_rna.jvalue(v)


def struct(s, depth=0):
    out = {"$type": s.bl_rna.identifier}
    for p in s.bl_rna.properties:
        if p.identifier in SKIP or (s.bl_rna.identifier, p.identifier) in dump_rna.MACHINE_PROPS:
            continue
        try:
            v = getattr(s, p.identifier)
        except Exception as e:  # noqa: BLE001
            out[p.identifier] = {"$error": f"{type(e).__name__}: {e}"}
            continue
        out[p.identifier] = ser(v, p, depth)
    return out


def mesh_exact(me):
    def fget(coll, attr, n, size, kind=float):
        buf = [kind(0)] * (n * size)
        coll.foreach_get(attr, buf)
        return [buf[i * size:(i + 1) * size] for i in range(n)] if size > 1 else buf

    nv, ne, np_, nl = len(me.vertices), len(me.edges), len(me.polygons), len(me.loops)
    attrs = []
    for a in me.attributes:
        n = {"POINT": nv, "EDGE": ne, "FACE": np_, "CORNER": nl}.get(a.domain, len(a.data))
        if a.data_type in ("FLOAT", "INT", "BOOLEAN", "INT8"):
            key = "value"
            size = 1
        elif a.data_type in ("FLOAT_VECTOR",):
            key, size = "vector", 3
        elif a.data_type == "FLOAT2":
            key, size = "vector", 2
        elif a.data_type == "INT32_2D":
            key, size = "value", 2
        elif a.data_type in ("FLOAT_COLOR", "BYTE_COLOR"):
            key, size = "color", 4
        else:
            attrs.append({"name": a.name, "domain": a.domain, "data_type": a.data_type})
            continue
        kind = bool if a.data_type == "BOOLEAN" else (int if a.data_type in ("INT", "INT8", "INT32_2D") else float)
        data = fget(a.data, key, n, size, kind)
        attrs.append({"name": a.name, "domain": a.domain, "data_type": a.data_type,
                      "data": [dump_rna.jvalue(x) for x in data]})
    return {
        "name": me.name,
        "positions": fget(me.vertices, "co", nv, 3),
        "edges": fget(me.edges, "vertices", ne, 2, int),
        "loop_start": fget(me.polygons, "loop_start", np_, 1, int),
        "loop_total": fget(me.polygons, "loop_total", np_, 1, int),
        "corner_verts": fget(me.loops, "vertex_index", nl, 1, int),
        "corner_edges": fget(me.loops, "edge_index", nl, 1, int),
        "attributes": attrs,
        "uv_layers": [{"name": u.name, "active": u.active, "active_render": u.active_render}
                      for u in me.uv_layers],
        "materials": [dump_rna.jvalue(m) for m in me.materials],
    }


def view3d():
    ws = bpy.data.workspaces["Layout"]
    screen = ws.screens[0]
    area = next(a for a in screen.areas if a.type == "VIEW_3D")
    space = area.spaces.active
    r3d = space.region_3d
    return {
        "workspace": ws.name,
        "screen": screen.name,
        "region_3d": {
            "view_perspective": r3d.view_perspective,
            "is_perspective": r3d.is_perspective,
            "is_orthographic_side_view": r3d.is_orthographic_side_view,
            "view_location": dump_rna.jvalue(r3d.view_location),
            "view_rotation": dump_rna.jvalue(r3d.view_rotation),
            "view_distance": dump_rna.jfloat(r3d.view_distance),
            "view_camera_zoom": dump_rna.jfloat(r3d.view_camera_zoom),
            "view_camera_offset": dump_rna.jvalue(r3d.view_camera_offset),
            "view_matrix": dump_rna.jvalue(r3d.view_matrix),
            "window_matrix": dump_rna.jvalue(r3d.window_matrix),
            "perspective_matrix": dump_rna.jvalue(r3d.perspective_matrix),
        },
        "space": {
            "lens": dump_rna.jfloat(space.lens),
            "clip_start": dump_rna.jfloat(space.clip_start),
            "clip_end": dump_rna.jfloat(space.clip_end),
            "camera": dump_rna.jvalue(space.camera),
            "use_local_camera": space.use_local_camera,
            "shading": struct(space.shading, 2),
            "overlay_show_overlays": space.overlay.show_overlays,
        },
        "area_size": [area.width, area.height],
        "region_size": [next(r for r in area.regions if r.type == "WINDOW").width,
                        next(r for r in area.regions if r.type == "WINDOW").height],
    }


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = None
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out = argv[i + 1]; i += 2
        else:
            raise SystemExit(f"dump_startup.py: unknown argument {argv[i]!r}")
    if not out:
        raise SystemExit("dump_startup.py: --out DIR is required")
    return out


COLLECTIONS = ["scenes", "objects", "meshes", "materials", "cameras", "lights", "worlds",
               "collections", "images", "node_groups", "libraries", "textures", "actions",
               "armatures", "pointclouds", "shape_keys"]


def main():
    out = parse_args()
    dump_rna.enable_addon_prefs()
    ctx = bpy.context
    data = {
        "blender": dump_rna.blender_meta(),
        "context": {
            "mode": ctx.mode,
            "active_object": ctx.view_layer.objects.active.name if ctx.view_layer.objects.active else None,
            "selected_objects": [o.name for o in ctx.view_layer.objects if o.select_get()],
        },
        "window": {
            "scene": ctx.window.scene.name,
            "view_layer": ctx.window.view_layer.name,
            "workspace": ctx.window.workspace.name,
            "screen": ctx.window.screen.name,
        },
        "view3d": view3d(),
        "ids": {},
        "meshes_exact": [],
    }
    del data["blender"]["property_types"], data["blender"]["subtypes"], data["blender"]["units"]
    for attr in COLLECTIONS:
        coll = getattr(bpy.data, attr)
        if len(coll):
            data["ids"][attr] = [struct(i) for i in coll]
    data["meshes_exact"] = [mesh_exact(me) for me in bpy.data.meshes]
    dump_rna.write_json(os.path.join(out, "startup.json"), data)
    print("dump_startup.py: " + ", ".join(f"{k}={len(v)}" for k, v in data["ids"].items()))


if __name__ == "__main__":
    main()
