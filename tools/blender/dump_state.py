# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender-side `GoldenState` dumper (ARCHITECTURE §20.3). WP 1.8.

Writes the JSON that `crates/frap-golden/src/state.rs` (`GoldenState`) deserialises, reading
exactly the properties listed in `tests/golden/props.toml` (the frap side reads the same list
through frap's RNA). Called by `golden_runner.py` after the case's statements ran; never run on
its own.

Floats are written as the shortest decimal that round-trips the f32 value (so fixtures stay
readable and diff well); non-finite floats as `{"$float": "nan" | "inf" | "-inf"}` (D-14).
"""

import math
import os
import struct

import bpy

# D-11: frap's factory startup has no grease-pencil material.
EXCLUDED_MATERIALS = {"Dots Stroke"}


def f32(x):
    """Shortest decimal that round-trips the f32 value of `x` (D-14 for non-finite)."""
    x = float(x)
    if math.isnan(x):
        return {"$float": "nan"}
    if math.isinf(x):
        return {"$float": "inf" if x > 0 else "-inf"}
    f = struct.unpack("<f", struct.pack("<f", x))[0]
    for p in range(1, 10):
        s = float("%.*g" % (p, f))
        if struct.unpack("<f", struct.pack("<f", s))[0] == f:
            return s
    return f


def fvec(v):
    return [f32(x) for x in v]


def resolve(base, path):
    """`a.b.c` attribute chain."""
    for part in path.split("."):
        base = getattr(base, part)
    return base


def items(base, path):
    """`coll/prop` -> [item.prop for item in coll]."""
    coll, prop = path.split("/", 1)
    return [resolve(it, prop.replace("/", ".")) for it in resolve(base, coll)]


def name_of(idblock):
    return None if idblock is None else idblock.name


def dump_object(ob, p):
    po = p["object"]
    mode = resolve(ob, po["rotation_mode"])
    if mode in ("QUATERNION", "AXIS_ANGLE"):
        rot_path = po["rotation"][mode]
    else:
        rot_path = po["rotation"]["EULER"]
    mw = resolve(ob, po["matrix_world"])
    return {
        "type": resolve(ob, po["type"]),
        "parent": name_of(resolve(ob, po["parent"])),
        "matrix_world": [fvec(mw[r]) for r in range(4)],
        "location": fvec(resolve(ob, po["location"])),
        "rotation_mode": mode,
        "rotation": fvec(resolve(ob, rot_path)),
        "scale": fvec(resolve(ob, po["scale"])),
        "modifiers": [
            [resolve(m, k) for k in po["modifier_props"]]
            for m in resolve(ob, po["modifiers"])
        ],
        "materials": [name_of(m) for m in items(ob, po["materials"])],
        "mode": resolve(ob, po["mode"]),
    }


def attribute_value(a, field):
    out = []
    for d in a.data:
        v = getattr(d, field)
        if isinstance(v, bool) or isinstance(v, int):
            out.append(v)
        elif isinstance(v, float):
            out.append(f32(v))
        elif isinstance(v, (str, bytes)):
            out.append(v.decode("utf-8", "replace") if isinstance(v, bytes) else v)
        else:
            flat = []
            for x in v:
                if hasattr(x, "__len__"):
                    flat.append(fvec(x))
                elif isinstance(x, float):
                    flat.append(f32(x))
                else:
                    flat.append(x)
            out.append(flat)
    return out


def dump_mesh(me, p, edit=False):
    if edit:
        # D-05 (M2, WP 2.2): in Edit Mode Blender's RNA lists the BMesh layers as `attributes`
        # (no `sharp_face`, …) and gives UV maps no data, while frap reads the edit mesh as a
        # mesh. The geometry of an edit mesh is dumped from a copy of the mesh written by
        # `update_from_editmode()` (the runner's D-05 normalisation), the edit state from the
        # mesh itself.
        copy = me.copy()
        try:
            state = dump_mesh(copy, p)
        finally:
            bpy.data.meshes.remove(copy)
        state.update(dump_edit_state(me, p))
        return state
    pm = p["mesh"]
    uv_names = [l.name for l in resolve(me, pm["uv_layers"])]
    attributes = {}
    for a in resolve(me, pm["attributes"]):
        if a.name.startswith(".") or a.name == "position" or a.name in uv_names:
            continue
        entry = {k: getattr(a, k) for k in pm["attribute_props"]}
        entry["data"] = attribute_value(a, pm["attribute_value"][a.data_type])
        attributes[a.name] = entry
    state = {
        "positions": [fvec(c) for c in items(me, pm["positions"])],
        "edges": [list(e) for e in items(me, pm["edges"])],
        "faces": [list(f) for f in items(me, pm["faces"])],
        "corner_normals": [fvec(n) for n in items(me, pm["corner_normals"])],
        "uv_layers": {
            l.name: [fvec(uv) for uv in items(l, pm["uv_value"])]
            for l in resolve(me, pm["uv_layers"])
        },
        "attributes": attributes,
        "selection": None,
        "active": None,
        "select_history": None,
    }
    return state


def dump_edit_state(me, p):
    """Selection, active element and selection history of a mesh in Edit Mode."""
    pm = p["mesh"]
    state = {}
    sel = {}
    for key, path in zip(("vert", "edge", "face"), pm["selection"]):
        sel[key] = [i for i, s in enumerate(items(me, path)) if s]
    state["selection"] = sel
    try:
        import bmesh

        bm = bmesh.from_edit_mesh(me)

        def el(e):
            kind = {"BMVert": "VERT", "BMEdge": "EDGE", "BMFace": "FACE"}[type(e).__name__]
            return {"type": kind, "index": e.index}

        act = bm.select_history.active
        state["active"] = None if act is None else el(act)
        state["select_history"] = [el(e) for e in bm.select_history]
    except Exception:  # noqa: BLE001 - mesh not in edit mode after all
        pass
    return state


def dump_material(mat, p):
    pm = p["material"]
    nodes = {}
    tree = mat.node_tree
    if tree is None:
        return {"nodes": {}, "links": []}
    sock_coll, sock_prop = pm["inputs"].split("/", 1)
    for n in resolve(mat, pm["nodes"]):
        entry = {k: resolve(n, k) for k in pm["node_props"]}
        inputs = {}
        for s in resolve(n, sock_coll):
            if not hasattr(s, sock_prop):
                continue
            v = getattr(s, sock_prop)
            if isinstance(v, bool) or isinstance(v, int) or isinstance(v, str):
                inputs[s.identifier] = v
            elif isinstance(v, float):
                inputs[s.identifier] = f32(v)
            elif hasattr(v, "name") and hasattr(v, "users"):
                inputs[s.identifier] = v.name
            elif v is None:
                inputs[s.identifier] = None
            else:
                inputs[s.identifier] = fvec(v)
        entry["inputs"] = inputs
        nodes[n.name] = entry
    links = sorted(
        [l.from_node.name, l.from_socket.identifier, l.to_node.name, l.to_socket.identifier]
        for l in resolve(mat, pm["links"])
    )
    return {"nodes": nodes, "links": links}


def dump_action(act):
    fcurves = []
    curves = list(getattr(act, "fcurves", []) or [])
    if not curves:
        for layer in getattr(act, "layers", []):
            for strip in layer.strips:
                for bag in getattr(strip, "channelbags", []):
                    curves.extend(bag.fcurves)
    for fc in curves:
        fcurves.append(
            {
                "data_path": fc.data_path,
                "array_index": fc.array_index,
                "keyframes": [
                    [fvec(k.co), k.interpolation, fvec(k.handle_left), fvec(k.handle_right)]
                    for k in fc.keyframe_points
                ],
            }
        )
    fcurves.sort(key=lambda c: (c["data_path"], c["array_index"]))
    slots = [s.identifier for s in getattr(act, "slots", [])]
    return {"data": {"slots": slots, "fcurves": fcurves}}


def dump_context(p):
    pc = p["context"]
    ctx = bpy.context
    mode = resolve(ctx, pc["mode"])
    return {
        "active": name_of(resolve(ctx, pc["active"])),
        "selected": [o.name for o in resolve(ctx, pc["selected"])],
        "mode": mode,
        "select_mode": list(resolve(ctx, pc["select_mode"])) if mode == "EDIT_MESH" else None,
        "cursor": fvec(resolve(ctx, pc["cursor"])),
    }


def png_size(path):
    with open(path, "rb") as f:
        head = f.read(24)
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        return 0, 0
    return struct.unpack(">II", head[16:24])


def dump(props, compare, lines, workdir):
    """The whole `GoldenState` as a dict."""
    edit_meshes = {ob.data.name for ob in bpy.data.objects if ob.mode == "EDIT" and ob.type == "MESH"}
    state = {
        "blender": {
            "version": bpy.app.version_string,
            "build_hash": bpy.app.build_hash.decode("ascii", "replace"),
        },
        "lines": lines,
        "objects": {ob.name: dump_object(ob, props) for ob in bpy.data.objects},
        "meshes": {},
        "evaluated": {},
        "materials": {},
        "actions": {},
        "context": dump_context(props),
        "files": {},
    }
    for kind, arg in compare:
        if kind == "mesh" and arg in bpy.data.meshes:
            state["meshes"][arg] = dump_mesh(bpy.data.meshes[arg], props, arg in edit_meshes)
        elif kind == "evaluated" and arg in bpy.data.objects:
            ob = bpy.data.objects[arg]
            dg = bpy.context.evaluated_depsgraph_get()
            ev = ob.evaluated_get(dg)
            me = ev.to_mesh()
            if me is not None:
                state["evaluated"][arg] = dump_mesh(me, props)
                ev.to_mesh_clear()
        elif kind == "materials":
            for mat in bpy.data.materials:
                if mat.name not in EXCLUDED_MATERIALS:
                    state["materials"][mat.name] = dump_material(mat, props)
        elif kind == "anim" and arg in bpy.data.actions:
            state["actions"][arg] = dump_action(bpy.data.actions[arg])
        elif kind == "glb":
            path = os.path.join(workdir, arg)
            if os.path.isfile(path):
                state["files"][arg] = {"kind": "gltf", "canonical": {"$file": os.path.abspath(path)}}
        elif kind == "image":
            path = os.path.join(workdir, arg)
            if os.path.isfile(path):
                w, h = png_size(path)
                state["files"][arg] = {
                    "kind": "image",
                    "width": w,
                    "height": h,
                    "pixels": {"$file": os.path.abspath(path)},
                }
    return state
