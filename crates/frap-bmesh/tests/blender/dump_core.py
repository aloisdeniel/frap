# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender 5.2 goldens for frap's BMesh core (WP 2.1, ARCHITECTURE §6.2, §22.4).

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup --python crates/frap-bmesh/tests/blender/dump_core.py -- \
        --out crates/frap-bmesh/tests/fixtures/core.blender.json [--filter <substring>]

Every case builds a mesh in object mode (a primitive operator or `from_pydata` with attributes,
vertex groups, hidden/selected/sharp/seam flags), dumps it (`setups[<setup>]`, once per setup:
setups are deterministic), sets the tool settings'
select mode, enters Edit Mode, applies a list of **steps** to the edit-mode BMesh through the
`bmesh` Python API (or `bpy.ops.mesh.*` selection operators), dumps BMesh state (`bm`: selection
history, active face, vertex and face normals, step results), leaves Edit Mode and dumps the
mesh again (`after`). frap's test (`crates/frap-bmesh/tests/core_vs_blender.rs`) rebuilds
`before` as a `frap_mesh::Mesh`, runs `BMesh::from_mesh`, replays the same steps with the frap
kernel, converts back with `to_mesh` and compares with `after` (T1: element order, every
attribute in order with values, positions/UVs within 1e-5).

Element references in steps are indices into the BMesh's *current* iteration order
(`ensure_lookup_table`), which is what frap's pool slots iterate in. Floats are written as the
Python `repr` of the stored f32 values.
"""

import json
import os
import sys

import bmesh
import bpy

# --------------------------------------------------------------------------- mesh dump

TOPOLOGY = ("position", ".edge_verts", ".corner_vert", ".corner_edge")


def _vals(attr):
    if attr.name in TOPOLOGY:
        return None
    t = attr.data_type
    d = attr.data
    if t == 'BOOLEAN':
        return [bool(x.value) for x in d]
    if t in ('FLOAT', 'INT', 'INT8'):
        return [x.value for x in d]
    if t in ('FLOAT2', 'FLOAT_VECTOR'):
        return [list(x.vector) for x in d]
    if t == 'FLOAT_COLOR':
        return [list(x.color) for x in d]
    if t == 'BYTE_COLOR':
        # The stored bytes (`color_srgb` is byte / 255).
        return [[round(c * 255.0) for c in x.color_srgb] for x in d]
    if t in ('INT16_2D', 'INT32_2D'):
        return [list(x.value) for x in d]
    if t == 'QUATERNION':
        return [list(x.value) for x in d]
    if t == 'STRING':
        return [x.value.decode() for x in d]
    return None


def mesh_dump(me):
    out = {
        "positions": [list(v.co) for v in me.vertices],
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
        "corner_edges": [l.edge_index for l in me.loops],
        "attributes": [
            {"name": a.name, "domain": a.domain, "data_type": a.data_type, "data": _vals(a)}
            for a in me.attributes
        ],
        "uv_active": me.uv_layers.active.name if me.uv_layers.active else None,
        "uv_render": next((l.name for l in me.uv_layers if l.active_render), None),
        # `Mesh.act_face` (-1 without an active face); frap's Mesh has no field for it yet.
        "act_face": me.polygons.active if len(me.polygons) else -1,
    }
    if any(len(v.groups) for v in me.vertices):
        out["deform"] = [[[g.group, g.weight] for g in v.groups] for v in me.vertices]
    return out


# --------------------------------------------------------------------------- mesh setups


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def prim(op, **kw):
    def build():
        getattr(bpy.ops.mesh, op)(**kw)
        return bpy.context.object
    return build


def new_object(name, verts, edges, faces):
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, edges, faces)
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    return ob


def grid_pydata(nx, ny, size=1.0):
    verts = [(x * size, y * size, 0.0) for y in range(ny + 1) for x in range(nx + 1)]
    faces = []
    for y in range(ny):
        for x in range(nx):
            a = y * (nx + 1) + x
            faces.append((a, a + 1, a + nx + 2, a + nx + 1))
    return verts, faces


def attrs_mesh():
    """A 3x2 quad grid plus a triangle, a loose edge and a loose vertex, with every attribute
    kind the BMesh conversion handles."""
    verts, faces = grid_pydata(3, 2)
    n = len(verts)
    verts += [(4.0, 0.0, 0.5), (5.0, 2.0, 0.0), (6.0, 2.0, 0.0), (7.0, 7.0, 7.0)]
    faces.append((3, n, 7))
    ob = new_object("Attrs", verts, [(n + 1, n + 2)], faces)
    me = ob.data
    nv, ne, nf, nc = len(me.vertices), len(me.edges), len(me.polygons), len(me.loops)
    a = me.attributes.new("my_flag", 'BOOLEAN', 'POINT')
    for i in range(nv):
        a.data[i].value = i % 3 == 0
    a = me.attributes.new("weight", 'FLOAT', 'EDGE')
    for i in range(ne):
        a.data[i].value = i * 0.25
    a = me.attributes.new("group", 'INT', 'FACE')
    for i in range(nf):
        a.data[i].value = 10 - i
    a = me.attributes.new("tag8", 'INT8', 'FACE')
    for i in range(nf):
        a.data[i].value = i - 2
    a = me.attributes.new("offset", 'FLOAT_VECTOR', 'POINT')
    for i in range(nv):
        a.data[i].vector = (i * 0.5, -i, 1.0)
    a = me.attributes.new("pair", 'INT32_2D', 'EDGE')
    for i in range(ne):
        a.data[i].value = (i, -i)
    a = me.attributes.new("Col", 'BYTE_COLOR', 'CORNER')
    for i in range(nc):
        a.data[i].color = ((i % 5) / 4.0, 0.5, 1.0 - (i % 3) / 2.0, 1.0)
    a = me.attributes.new("Paint", 'FLOAT_COLOR', 'POINT')
    for i in range(nv):
        a.data[i].color = (i / nv, 0.25, 0.75, 0.5)
    a = me.attributes.new("rot", 'QUATERNION', 'FACE')
    for i in range(nf):
        a.data[i].value = (1.0, 0.0, 0.0, 0.0) if i % 2 else (0.0, 1.0, 0.0, 0.0)
    a = me.attributes.new("label", 'STRING', 'FACE')
    for i in range(nf):
        a.data[i].value = b"f%d" % i
    uv = me.uv_layers.new(name="UVMap")
    for i in range(nc):
        uv.data[i].uv = ((i % 4) * 0.25, (i // 4) * 0.125)
    uv2 = me.uv_layers.new(name="Second")
    for i in range(nc):
        uv2.data[i].uv = (1.0 - (i % 4) * 0.25, 0.5)
    uv2.data[2].pin_uv = True
    me.uv_layers.active = uv2
    for i in (1, 6):
        me.vertices[i].hide = True
    for i in (2, 5):
        me.edges[i].use_seam = True
    for i in (0, 3, 4):
        me.edges[i].use_edge_sharp = True
    me.polygons[1].material_index = 2
    me.polygons[4].material_index = 1
    for p in me.polygons:
        p.use_smooth = p.index % 2 == 0
    for v in me.vertices:
        v.select = v.index in (0, 4, 5, 8, 9)
    for e in me.edges:
        e.select = e.index in (7,)
    for p in me.polygons:
        p.select = p.index == 3
    me.polygons[2].hide = True
    vg = ob.vertex_groups.new(name="G")
    vg.add([0, 1, 2, 5], 0.5, 'REPLACE')
    vg2 = ob.vertex_groups.new(name="H")
    vg2.add([2, 3], 1.0, 'REPLACE')
    return ob


def pydata_verts(n):
    def build():
        return new_object("Verts", [(float(i), 0.0, 0.0) for i in range(n)], [], [])
    return build


def grid_obj(nx, ny, uv=True, color=True):
    def build():
        verts, faces = grid_pydata(nx, ny)
        ob = new_object("Grid", verts, [], faces)
        me = ob.data
        if uv:
            l = me.uv_layers.new(name="UVMap")
            for p in me.polygons:
                for li in p.loop_indices:
                    co = me.vertices[me.loops[li].vertex_index].co
                    l.data[li].uv = (co.x / nx, co.y / ny)
        if color:
            a = me.attributes.new("Col", 'FLOAT_COLOR', 'CORNER')
            for i in range(len(me.loops)):
                a.data[i].color = (i / len(me.loops), 1.0 - i / len(me.loops), 0.5, 1.0)
            b = me.attributes.new("w", 'FLOAT', 'POINT')
            for i in range(len(me.vertices)):
                b.data[i].value = i * 0.1
        return ob
    return build


def tube():
    """An open quad tube (cube without caps) plus a cap n-gon on top: hub and boundary
    edge loops."""
    v = [(-1, -1, -1), (-1, -1, 1), (-1, 1, -1), (-1, 1, 1), (1, -1, -1), (1, -1, 1), (1, 1, -1),
         (1, 1, 1), (0, -1, 1), (0, -1, -1), (0, 1, 1), (0, 1, -1)]
    f = [(0, 1, 3, 2), (2, 3, 10, 11), (11, 10, 7, 6), (6, 7, 5, 4), (4, 5, 8, 9), (9, 8, 1, 0),
         (1, 8, 5, 7, 10, 3)]
    return new_object("Tube", [tuple(map(float, x)) for x in v], [], f)


SETUPS = {
    "cube": prim("primitive_cube_add"),
    "cube_nouv": prim("primitive_cube_add", size=3.0, calc_uvs=False),
    "plane": prim("primitive_plane_add"),
    "circle_ngon": prim("primitive_circle_add", fill_type='NGON'),
    "circle_trifan": prim("primitive_circle_add", vertices=7, fill_type='TRIFAN'),
    "cone": prim("primitive_cone_add"),
    "cylinder": prim("primitive_cylinder_add"),
    "grid": prim("primitive_grid_add", x_subdivisions=3, y_subdivisions=5),
    "ico": prim("primitive_ico_sphere_add"),
    "monkey": prim("primitive_monkey_add"),
    "torus": prim("primitive_torus_add"),
    "attrs": attrs_mesh,
    "verts6": pydata_verts(6),
    "verts500": pydata_verts(500),
    "verts513": pydata_verts(513),
    "grid4": grid_obj(4, 4),
    "grid3_plain": grid_obj(3, 3, uv=False, color=False),
    "tube": tube,
}

# --------------------------------------------------------------------------- steps

V, E, F = "V", "E", "F"


def lut(bm):
    bm.verts.ensure_lookup_table()
    bm.edges.ensure_lookup_table()
    bm.faces.ensure_lookup_table()


def el(bm, ref):
    t, i = ref
    return {V: bm.verts, E: bm.edges, F: bm.faces}[t][i]


def ref_of(e):
    if isinstance(e, bmesh.types.BMVert):
        return [V, e.index]
    if isinstance(e, bmesh.types.BMEdge):
        return [E, e.index]
    return [F, e.index]


def index_refs(bm, elems):
    bm.verts.index_update()
    bm.edges.index_update()
    bm.faces.index_update()
    return [ref_of(e) for e in elems]


def run_step(ob, bm, s):
    """Applies one step; returns (bm, result or None)."""
    lut(bm)
    op = s["op"]
    if op == "vert_new":
        bm.verts.new(s["co"])
    elif op == "vert_remove":
        bm.verts.remove(el(bm, [V, s["v"]]))
    elif op == "edge_new":
        bm.edges.new([el(bm, [V, i]) for i in s["verts"]])
    elif op == "edge_remove":
        bm.edges.remove(el(bm, [E, s["e"]]))
    elif op == "face_new":
        bm.faces.new([el(bm, [V, i]) for i in s["verts"]])
    elif op == "face_remove":
        bm.faces.remove(el(bm, [F, s["f"]]))
    elif op == "co":
        el(bm, [V, s["v"]]).co = s["co"]
    elif op == "delete":
        bmesh.ops.delete(bm, geom=[el(bm, r) for r in s["geom"]], context=s["context"])
    elif op == "duplicate":
        r = bmesh.ops.duplicate(bm, geom=[el(bm, x) for x in s["geom"]],
                                use_select_history=s.get("use_select_history", False),
                                use_edge_flip_from_face=s.get("use_edge_flip_from_face", False))
        geom = index_refs(bm, r["geom"])
        lut(bm)
        def pairs(m):
            out = []
            for k, v in m.items():
                out.append([k.index, v.index])
            return sorted(out)
        return bm, {
            "geom": geom,
            "boundary_map": pairs(r["boundary_map"]),
            "isovert_map": pairs(r["isovert_map"]),
        }
    elif op == "edge_split":
        # "end": which of the edge's vertices the factor starts from.
        e = el(bm, [E, s["e"]])
        bmesh.utils.edge_split(e, e.verts[s["end"]], s["fac"])
    elif op == "face_split":
        # "a"/"b": corner indices of the face (from its first loop).
        f = el(bm, [F, s["f"]])
        bmesh.utils.face_split(f, f.verts[s["a"]], f.verts[s["b"]])
    elif op == "face_join":
        bmesh.utils.face_join([el(bm, [F, i]) for i in s["faces"]], s.get("remove", True))
    elif op == "vert_separate":
        # "edges": positions in the vertex's disk cycle (`link_edges`).
        v = el(bm, [V, s["v"]])
        r = bmesh.utils.vert_separate(v, [v.link_edges[i] for i in s["edges"]])
        return bm, index_refs(bm, r)
    elif op == "face_flip":
        bmesh.utils.face_flip(el(bm, [F, s["f"]]))
    elif op == "vert_splice":
        bmesh.utils.vert_splice(el(bm, [V, s["v"]]), el(bm, [V, s["target"]]))
    elif op == "select_set":
        el(bm, s["elem"]).select_set(s["value"])
    elif op == "select_flush_mode":
        bm.select_flush_mode()
    elif op == "select_flush":
        bm.select_flush(s["value"])
    elif op == "select_mode":
        bm.select_mode = set(s["mode"])
    elif op == "history_add":
        bm.select_history.add(el(bm, s["elem"]))
    elif op == "normal_update":
        bm.normal_update()
    elif op == "loop_interp":
        f = el(bm, [F, s["f"]])
        f.loops[s["corner"]].copy_from_face_interp(el(bm, [F, s["src"]]), s["vert"], False)
    elif op == "vert_interp":
        el(bm, [V, s["v"]]).copy_from_vert_interp((el(bm, [V, s["a"]]), el(bm, [V, s["b"]])),
                                                  s["fac"])
    elif op == "deselect_all":
        for seq in (bm.verts, bm.edges, bm.faces):
            for x in seq:
                x.select = False
        bm.select_history.clear()
    elif op == "bpy":
        # A selection operator on the edit mesh (flush the BMesh first, re-fetch after).
        bmesh.update_edit_mesh(ob.data)
        fn = getattr(bpy.ops.mesh, s["name"])
        fn(**s.get("kw", {}))
        bm = bmesh.from_edit_mesh(ob.data)
    elif op == "selected":
        lut(bm)
        return bm, {
            "verts": [v.index for v in bm.verts if v.select],
            "edges": [e.index for e in bm.edges if e.select],
            "faces": [f.index for f in bm.faces if f.select],
        }
    else:
        raise ValueError(op)
    return bm, None


def bm_dump(bm, normals):
    bm.verts.index_update()
    bm.edges.index_update()
    bm.faces.index_update()
    out = {
        "history": [ref_of(e) for e in bm.select_history],
        "act_face": bm.faces.active.index if bm.faces.active else None,
        "select_mode": sorted(bm.select_mode),
        "counts": [len(bm.verts), len(bm.edges), len(bm.faces)],
    }
    if normals:
        out["vert_normals"] = [list(v.normal) for v in bm.verts]
        out["face_normals"] = [list(f.normal) for f in bm.faces]
    return out


# --------------------------------------------------------------------------- cases

def sel(t, *idx):
    return [[t, i] for i in idx]


CASES = []


def case(name, setup, steps, mode=("VERT",), normals=False):
    CASES.append({"name": name, "setup": setup, "steps": steps, "mode": list(mode),
                  "normals": normals})


# Round trips: edit-mode enter + exit (attribute order, flags, pins, groups).
for s in ("cube", "cube_nouv", "plane", "circle_ngon", "circle_trifan", "cone", "cylinder",
          "grid", "ico", "monkey", "torus", "attrs", "grid4", "tube"):
    case("roundtrip/" + s, s, [])

# Mempool order [MESH App. B.6] and the reset-on-empty rule of BLI_mempool_free.
case("pool/app_b6", "verts6", [
    {"op": "vert_remove", "v": 2}, {"op": "vert_remove", "v": 3},
    {"op": "vert_new", "co": [100.0, 0.0, 0.0]}, {"op": "vert_new", "co": [200.0, 0.0, 0.0]},
    {"op": "vert_new", "co": [300.0, 0.0, 0.0]}])
for n, extra in (("verts500", 0), ("verts500", 84), ("verts500", 85), ("verts513", 0)):
    steps = [{"op": "vert_new", "co": [-1.0, float(k), 0.0]} for k in range(extra)]
    total = int(n[5:]) + extra
    steps += [{"op": "vert_remove", "v": 0} for _ in range(total)]
    steps += [{"op": "vert_new", "co": [float(k), 1.0, 0.0]} for k in range(3)]
    case("pool/%s_plus%d_empty" % (n, extra), n, steps)

# bmesh.ops.delete contexts (cube, everything selected so the flush matters).
TOP = 5
for ctx in ("VERTS", "EDGES", "FACES_ONLY", "EDGES_FACES", "FACES", "FACES_KEEP_BOUNDARY",
            "TAGGED_ONLY"):
    case("delete/cube_top_face_" + ctx, "cube",
         [{"op": "delete", "geom": sel(F, TOP), "context": ctx}])
case("delete/cube_edge0_EDGES", "cube", [{"op": "delete", "geom": sel(E, 0), "context": "EDGES"}])
case("delete/cube_edge0_EDGES_FACES", "cube",
     [{"op": "delete", "geom": sel(E, 0), "context": "EDGES_FACES"}])
case("delete/cube_vert0_VERTS", "cube", [{"op": "delete", "geom": sel(V, 0), "context": "VERTS"}])
case("delete/cube_mixed_TAGGED_ONLY", "cube",
     [{"op": "delete", "geom": sel(V, 7) + sel(E, 3, 4) + sel(F, 0, 2), "context": "TAGGED_ONLY"}])
case("delete/grid4_region_FACES_then_refill", "grid4", [
    {"op": "delete", "geom": sel(F, 5, 6, 9, 10), "context": "FACES"},
    {"op": "vert_new", "co": [1.5, 1.5, 1.0]},
    {"op": "face_new", "verts": [5, 6, 13]},
    {"op": "face_new", "verts": [6, 9, 13]}])
case("delete/monkey_verts_then_duplicate", "monkey", [
    {"op": "delete", "geom": sel(V, 0, 10, 20, 30, 40), "context": "VERTS"},
    {"op": "duplicate", "geom": sel(F, 3, 4, 5)}])
case("delete/attrs_FACES_KEEP_BOUNDARY", "attrs",
     [{"op": "delete", "geom": sel(F, 0, 1), "context": "FACES_KEEP_BOUNDARY"}])

# bmesh.ops.duplicate.
case("duplicate/cube_top_face", "cube", [{"op": "duplicate", "geom": sel(F, TOP)}])
case("duplicate/cube_two_faces_history", "cube", [
    {"op": "deselect_all"},
    {"op": "select_set", "elem": [F, 1], "value": True},
    {"op": "select_set", "elem": [F, 2], "value": True},
    {"op": "history_add", "elem": [F, 1]},
    {"op": "history_add", "elem": [F, 2]},
    {"op": "duplicate", "geom": sel(F, 1, 2), "use_select_history": True}])
case("duplicate/cube_edges_and_vert", "cube",
     [{"op": "duplicate", "geom": sel(E, 0, 1, 5) + sel(V, 7)}])
case("duplicate/grid4_edge_flip", "grid4",
     [{"op": "duplicate", "geom": sel(E, 0, 1, 2, 3), "use_edge_flip_from_face": True}])
case("duplicate/attrs_all", "attrs",
     [{"op": "duplicate", "geom": sel(F, 0, 1, 3, 4, 5, 6) + sel(E, 1) + sel(V, 15)}])

# Kernel through bmesh.utils (SEMV + interpolation, SFME, separate, flip, join, splice).
case("kernel/grid4_edge_split", "grid4",
     [{"op": "edge_split", "e": 5, "end": 0, "fac": 0.25},
      {"op": "edge_split", "e": 12, "end": 1, "fac": 0.5},
      {"op": "edge_split", "e": 41, "end": 0, "fac": 0.5}])
case("kernel/attrs_edge_split", "attrs",
     [{"op": "edge_split", "e": 3, "end": 1, "fac": 0.3},
      {"op": "edge_split", "e": 0, "end": 0, "fac": 0.75},
      {"op": "edge_split", "e": 21, "end": 0, "fac": 0.5}])
case("kernel/grid4_face_split", "grid4",
     [{"op": "face_split", "f": 5, "a": 0, "b": 2},
      {"op": "face_split", "f": 0, "a": 1, "b": 3},
      {"op": "edge_split", "e": 40, "end": 1, "fac": 0.5},
      {"op": "face_split", "f": 16, "a": 0, "b": 2}])
case("kernel/tube_ngon_face_split", "tube", [{"op": "face_split", "f": 6, "a": 1, "b": 4},
                                             {"op": "face_split", "f": 6, "a": 0, "b": 2}])
case("kernel/cube_vert_separate", "cube",
     [{"op": "vert_separate", "v": 0, "edges": [0, 1]}])
case("kernel/tube_vert_separate_two", "tube",
     [{"op": "vert_separate", "v": 1, "edges": [0, 2]}])
case("kernel/grid4_vert_separate_all", "grid4", [{"op": "vert_separate", "v": 12, "edges": []}])
case("kernel/cube_face_flip", "cube",
     [{"op": "face_flip", "f": 2}, {"op": "face_flip", "f": 5}])
case("kernel/cube_face_join", "cube", [{"op": "face_join", "faces": [1, 2]}])
case("kernel/grid4_face_join_region", "grid4",
     [{"op": "face_join", "faces": [5, 6, 9, 10]}])
case("kernel/grid4_face_join_keep", "grid4",
     [{"op": "face_join", "faces": [0, 1], "remove": False}])
case("kernel/verts_splice", "verts6", [{"op": "vert_splice", "v": 1, "target": 4}])
case("kernel/grid4_new_geometry", "grid4", [
    {"op": "vert_new", "co": [2.0, 2.0, 1.0]},
    {"op": "edge_new", "verts": [12, 25]},
    {"op": "face_new", "verts": [0, 25, 24]},
    {"op": "face_remove", "f": 3},
    {"op": "edge_remove", "e": 0}])

# Custom data interpolation.
case("interp/grid4_loop_and_vert", "grid4", [
    {"op": "co", "v": 6, "co": [1.3, 0.8, 0.4]},
    {"op": "normal_update"},
    {"op": "loop_interp", "f": 5, "corner": 1, "src": 0, "vert": True},
    {"op": "loop_interp", "f": 10, "corner": 0, "src": 10, "vert": False},
    {"op": "vert_interp", "v": 7, "a": 0, "b": 24, "fac": 0.3}])
case("interp/attrs_vert_interp", "attrs", [
    {"op": "vert_interp", "v": 3, "a": 0, "b": 5, "fac": 0.6},
    {"op": "vert_interp", "v": 4, "a": 2, "b": 3, "fac": 1.0}])

# Normals (BM_mesh_normals_update).
case("normals/monkey_moved", "monkey", [
    {"op": "co", "v": 5, "co": [0.3, -0.2, 0.9]}, {"op": "normal_update"}], normals=True)
case("normals/torus", "torus", [{"op": "normal_update"}], normals=True)
case("normals/attrs", "attrs", [{"op": "normal_update"}], normals=True)

# Selection: flushing and select_set in each mode.
for mode in ("VERT", "EDGE", "FACE"):
    case("select/grid4_flush_" + mode, "grid4", [
        {"op": "deselect_all"},
        {"op": "select_set", "elem": [V, 6], "value": True},
        {"op": "select_set", "elem": [V, 7], "value": True},
        {"op": "select_set", "elem": [V, 11], "value": True},
        {"op": "select_set", "elem": [V, 12], "value": True},
        {"op": "select_set", "elem": [E, 20], "value": True},
        {"op": "select_set", "elem": [F, 15], "value": True},
        {"op": "select_flush_mode"},
        {"op": "select_set", "elem": [F, 15], "value": False},
        {"op": "select_set", "elem": [E, 3], "value": False},
        {"op": "selected"}], mode=(mode,))
case("select/grid4_flush_bool", "grid4", [
    {"op": "deselect_all"},
    {"op": "select_set", "elem": [V, 0], "value": True},
    {"op": "select_set", "elem": [V, 1], "value": True},
    {"op": "select_set", "elem": [V, 5], "value": True},
    {"op": "select_set", "elem": [V, 6], "value": True},
    {"op": "select_flush", "value": True},
    {"op": "selected"},
    {"op": "select_set", "elem": [V, 6], "value": False},
    {"op": "select_flush", "value": False},
    {"op": "selected"}])
case("select/attrs_hidden", "attrs", [
    {"op": "select_set", "elem": [F, 2], "value": True},
    {"op": "select_set", "elem": [E, 9], "value": True},
    {"op": "select_flush_mode"},
    {"op": "selected"}])

# Selection mode conversion (mesh.select_mode, [MESH §3.3]).
for old, new, expand in (("VERT", "EDGE", False), ("VERT", "FACE", False),
                         ("VERT", "EDGE", True), ("VERT", "FACE", True),
                         ("EDGE", "FACE", True), ("EDGE", "VERT", True),
                         ("FACE", "EDGE", True), ("FACE", "VERT", True),
                         ("FACE", "VERT", False), ("EDGE", "FACE", False)):
    pre = {"VERT": [{"op": "select_set", "elem": [V, i], "value": True} for i in (6, 7, 12)],
           "EDGE": [{"op": "select_set", "elem": [E, i], "value": True} for i in (11, 13, 21)],
           "FACE": [{"op": "select_set", "elem": [F, i], "value": True} for i in (5, 6, 9)]}[old]
    case("selmode/grid4_%s_to_%s%s" % (old, new, "_expand" if expand else ""), "grid4",
         [{"op": "deselect_all"}] + pre + [
             {"op": "select_flush_mode"},
             {"op": "bpy", "name": "select_mode", "kw": {"type": new, "use_expand": expand}},
             {"op": "selected"}], mode=(old,))

# Walkers through the selection operators (edge select mode so flushing adds no edges).
def walk_query(kind, start, kw=None):
    t = {"loop": E, "ring": E, "island": F, "shell": V}[kind]
    name = {"loop": "select_edge_loop_multi", "ring": "select_edge_ring_multi",
            "island": "select_linked", "shell": "select_linked"}[kind]
    return [{"op": "deselect_all"}, {"op": "select_set", "elem": [t, start], "value": True},
            {"op": "bpy", "name": name, "kw": kw or {}}, {"op": "selected"}]


WALKS = [
    ("cube", "loop", [0, 5], {"delimit_edge_loop": set()}),
    ("grid", "loop", [0, 7, 13, 30], {"delimit_edge_loop": set()}),
    ("grid", "loop", [0, 7], {"delimit_edge_loop": {'OUTER_CORNERS', 'NGONS'}}),
    ("cylinder", "loop", [0, 1, 2, 60], {"delimit_edge_loop": set()}),
    ("tube", "loop", [0, 2, 4, 11], {"delimit_edge_loop": set()}),
    ("tube", "loop", [2, 4], {"delimit_edge_loop": {'OUTER_CORNERS', 'NGONS'}}),
    ("monkey", "loop", [0, 100, 250], {"delimit_edge_loop": set()}),
    ("torus", "loop", [0, 1], {"delimit_edge_loop": set()}),
    ("cube", "ring", [0], {"delimit_edge_ring": set()}),
    ("grid", "ring", [0, 7, 13], {"delimit_edge_ring": set()}),
    ("tube", "ring", [0, 2, 4], {"delimit_edge_ring": set()}),
    ("tube", "ring", [0, 2], {"delimit_edge_ring": {'NGONS'}}),
    ("cylinder", "ring", [0, 5], {"delimit_edge_ring": set()}),
    ("torus", "ring", [3], {"delimit_edge_ring": set()}),
    ("monkey", "ring", [40, 200], {"delimit_edge_ring": set()}),
    ("monkey", "island", [0, 400], {"delimit": set()}),
    ("attrs", "island", [0], {"delimit": {'SEAM'}}),
    ("attrs", "island", [0], {"delimit": {'SHARP'}}),
    ("attrs", "island", [0], {"delimit": {'MATERIAL'}}),
    ("attrs", "island", [0], {"delimit": {'UV'}}),
    ("monkey", "shell", [0, 300], {"delimit": set()}),
    ("attrs", "shell", [0, 13], {"delimit": set()}),
]
for setup, kind, starts, kw in WALKS:
    mode = {"loop": ("EDGE",), "ring": ("EDGE",), "island": ("FACE",), "shell": ("VERT",)}[kind]
    tag = "_".join(sorted(next(iter(kw.values())))) or "none"
    for s in starts:
        case("walk/%s_%s_%d_%s" % (setup, kind, s, tag.lower()), setup,
             walk_query(kind, s, kw), mode=mode)


# --------------------------------------------------------------------------- driver

def _jsonable(x):
    if isinstance(x, set):
        return sorted(x)
    raise TypeError(x)


def run_case(c):
    reset()
    ob = SETUPS[c["setup"]]()
    bpy.context.view_layer.objects.active = ob
    for o in bpy.context.view_layer.objects:
        o.select_set(o == ob)
    before = mesh_dump(ob.data)
    ts = bpy.context.scene.tool_settings
    ts.mesh_select_mode = tuple(m in c["mode"] for m in ("VERT", "EDGE", "FACE"))
    bpy.ops.object.mode_set(mode='EDIT')
    bm = bmesh.from_edit_mesh(ob.data)
    results = []
    for s in c["steps"]:
        bm, r = run_step(ob, bm, s)
        results.append(r)
    bmd = bm_dump(bm, c["normals"])
    bmd["results"] = results
    bmesh.update_edit_mesh(ob.data)
    bpy.ops.object.mode_set(mode='OBJECT')
    after = mesh_dump(ob.data)
    steps = json.loads(json.dumps(c["steps"], default=_jsonable))
    out = {"setup": c["setup"], "mode": c["mode"], "steps": steps, "bm": bmd}
    # Walker cases are compared through their selection results only (keeps the file small).
    if not c["name"].startswith("walk/"):
        out["after"] = after
    return before, out


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = None
    flt = None
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out = argv[i + 1]
            i += 2
        elif argv[i] == "--filter":
            flt = argv[i + 1]
            i += 2
        else:
            i += 1
    data = {"blender": bpy.app.version_string, "build_hash": bpy.app.build_hash.decode(),
            "setups": {}, "cases": {}}
    for c in CASES:
        if flt and flt not in c["name"]:
            continue
        before, result = run_case(c)
        # Setups are deterministic: one `before` dump per setup.
        prev = data["setups"].setdefault(c["setup"], before)
        assert prev == before, "setup %s is not deterministic" % c["setup"]
        data["cases"][c["name"]] = result
    text = json.dumps(data, separators=(",", ":"), default=_jsonable)
    with open(out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)
        fh.write("\n")
    print("wrote %d cases to %s" % (len(data["cases"]), out))


main()
