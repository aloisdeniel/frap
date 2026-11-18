# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender 5.2 goldens for frap's UV core (WP 3.3, ARCHITECTURE §22.5, [MESH App. B.5]).

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup --python crates/frap-uv/tests/blender/dump_uv.py -- \
        --out crates/frap-uv/tests/fixtures/uv.blender.json

Every case builds a mesh (a primitive operator), optionally marks seams, enters Edit Mode with
everything selected, dumps the edit-mode BMesh (`before`: positions, edges, face corners in
`l_first` order, corner edges, seams, pins, face normals and areas, the active UV map; stored once
per setup in `setups`, except for the pinned cases), runs a list of
`bpy.ops.uv.*` calls headless (no 3D view, no UV editor: the same context as App. B.5) and dumps
the corner UVs afterwards (`after`). frap's tests (`crates/frap-uv/tests/*.rs`) rebuild the
mesh as plain arrays and replay the operator pipelines (`uvedit_unwrap_ops.cc`) on top of
`frap_uv`. Floats are the Python `repr` of the stored f32 values.
"""

import json
import math
import sys

import bmesh
import bpy


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def dump(ob):
    bm = bmesh.from_edit_mesh(ob.data)
    bm.verts.ensure_lookup_table()
    bm.edges.ensure_lookup_table()
    bm.faces.ensure_lookup_table()
    uv = bm.loops.layers.uv.active
    pin = bm.loops.layers.bool.get(".pn.UVMap")
    faces = []
    corner_edges = []
    uvs = []
    pins = []
    for f in bm.faces:
        faces.append([l.vert.index for l in f.loops])
        corner_edges.extend(l.edge.index for l in f.loops)
        if uv is not None:
            uvs.extend(list(l[uv].uv) for l in f.loops)
        pins.extend(bool(pin is not None and l[pin]) for l in f.loops)
    return {
        "pins": pins,
        "positions": [list(v.co) for v in bm.verts],
        "edges": [[e.verts[0].index, e.verts[1].index] for e in bm.edges],
        "faces": faces,
        "corner_edges": corner_edges,
        "seams": [e.index for e in bm.edges if e.seam],
        "face_normals": [list(f.normal) for f in bm.faces],
        "face_areas": [f.calc_area() for f in bm.faces],
        "uvs": uvs,
    }


def seams_cube_standard(bm):
    """The 7 seams of App. B.5 (edge indices of `primitive_cube_add`)."""
    return [0, 2, 3, 5, 6, 9, 11]


def seams_cylinder(bm):
    """Both cap rings plus the first vertical edge."""
    out = []
    vertical = None
    for e in bm.edges:
        z0, z1 = e.verts[0].co.z, e.verts[1].co.z
        if abs(z0 - z1) < 1e-6:
            out.append(e.index)
        elif vertical is None:
            vertical = e.index
    out.append(vertical)
    return sorted(out)


def seams_meridian(bm):
    """A pole-to-pole meridian in the +X half plane (`y == 0`, `x >= 0`)."""
    out = []
    for e in bm.edges:
        a, b = e.verts[0].co, e.verts[1].co
        if abs(a.y) < 1e-5 and abs(b.y) < 1e-5 and a.x > -1e-5 and b.x > -1e-5:
            out.append(e.index)
    return out


def seams_tube_vertical(bm):
    """The first vertical edge of an open tube."""
    for e in bm.edges:
        if abs(e.verts[0].co.z - e.verts[1].co.z) > 1e-6:
            return [e.index]
    return []


def uv_sphere(segments=12, rings=6):
    """A UV sphere from `from_pydata` (Blender's `primitive_uv_sphere_add` does not produce a
    reproducible edge and face order across sessions, see deviations D-19), with a UV map."""
    verts = [(0.0, 0.0, 1.0)]
    for r in range(1, rings):
        theta = math.pi * r / rings
        for s in range(segments):
            phi = 2.0 * math.pi * s / segments
            verts.append((math.sin(theta) * math.cos(phi), math.sin(theta) * math.sin(phi),
                          math.cos(theta)))
    verts.append((0.0, 0.0, -1.0))
    south = len(verts) - 1
    ring = lambda r, s: 1 + (r - 1) * segments + s % segments
    faces = [(0, ring(1, s), ring(1, s + 1)) for s in range(segments)]
    for r in range(1, rings - 1):
        for s in range(segments):
            faces.append((ring(r, s), ring(r + 1, s), ring(r + 1, s + 1), ring(r, s + 1)))
    faces += [(ring(rings - 1, s + 1), ring(rings - 1, s), south) for s in range(segments)]
    me = bpy.data.meshes.new("Sphere")
    me.from_pydata(verts, [], faces)
    me.uv_layers.new(name="UVMap")
    ob = bpy.data.objects.new("Sphere", me)
    bpy.context.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)


SETUPS = {
    "cube": (lambda: bpy.ops.mesh.primitive_cube_add(), None),
    "cube_seams": (lambda: bpy.ops.mesh.primitive_cube_add(), seams_cube_standard),
    "cyl8_seams": (lambda: bpy.ops.mesh.primitive_cylinder_add(vertices=8), seams_cylinder),
    "cyl8": (lambda: bpy.ops.mesh.primitive_cylinder_add(vertices=8), None),
    "sphere": (uv_sphere, None),
    "sphere_meridian": (uv_sphere, seams_meridian),
    "tube": (
        lambda: bpy.ops.mesh.primitive_cylinder_add(vertices=10, end_fill_type='NOTHING'),
        None,
    ),
    "tube_seam": (
        lambda: bpy.ops.mesh.primitive_cylinder_add(vertices=10, end_fill_type='NOTHING'),
        seams_tube_vertical,
    ),
    "circle_ngon": (
        lambda: bpy.ops.mesh.primitive_circle_add(vertices=12, fill_type='NGON'),
        None,
    ),
    "monkey": (lambda: bpy.ops.mesh.primitive_monkey_add(), None),
    "grid": (
        lambda: bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3),
        None,
    ),
}

# (case name, setup, [(op, kwargs), ...]).
#
# NOTE: `bpy.ops.uv.unwrap()` takes every property it is not given from the scene's tool
# settings (`unwrap_options_sync_toolsettings`): in the factory startup file that is
# `method='ANGLE_BASED'` and `fill_holes=True`, not the RNA defaults (CONFORMAL, False). On
# developable charts with one boundary (the seamed cube, App. B.5) that makes no difference;
# every other LSCM case passes `method='CONFORMAL'` (and the annulus `fill_holes`) explicitly.
# `tube_unwrap_toolsettings` records the effective defaults on a chart where they matter.
CONF = {"method": "CONFORMAL"}
CASES = [
    ("cube_unwrap_seams", "cube_seams", [("unwrap", {})]),
    ("cube_unwrap_seams_conformal", "cube_seams", [("unwrap", CONF)]),
    ("cube_unwrap_seams_angle", "cube_seams", [("unwrap", {"method": "ANGLE_BASED"})]),
    ("cube_unwrap_seams_margin0", "cube_seams", [("unwrap", {**CONF, "margin": 0.0})]),
    ("cube_unwrap_noseams", "cube", [("unwrap", CONF)]),
    ("cube_smart_project", "cube", [("smart_project", {})]),
    ("cube_cube_project", "cube", [("cube_project", {})]),
    ("cube_cylinder_project", "cube", [("cylinder_project", {})]),
    ("cube_sphere_project", "cube", [("sphere_project", {})]),
    ("cube_cube_project_average", "cube", [("cube_project", {}), ("average_islands_scale", {})]),
    ("cube_smart_pack", "cube", [("smart_project", {}), ("pack_islands", {})]),
    ("cyl8_unwrap_seams", "cyl8_seams", [("unwrap", CONF)]),
    ("cyl8_smart_project", "cyl8", [("smart_project", {})]),
    (
        "cyl8_smart_average_shear",
        "cyl8",
        [("smart_project", {}), ("average_islands_scale", {"scale_uv": True, "shear": True})],
    ),
    ("cyl8_cylinder_project", "cyl8", [("cylinder_project", {})]),
    ("sphere_cylinder_project", "sphere", [("cylinder_project", {})]),
    ("sphere_sphere_project", "sphere", [("sphere_project", {})]),
    ("sphere_sphere_project_fan", "sphere", [("sphere_project", {"pole": "FAN"})]),
    (
        "sphere_sphere_project_object",
        "sphere",
        [("sphere_project", {"direction": "ALIGN_TO_OBJECT"})],
    ),
    ("sphere_unwrap_meridian", "sphere_meridian", [("unwrap", CONF)]),
    ("tube_unwrap", "tube", [("unwrap", {**CONF, "fill_holes": False})]),
    ("tube_unwrap_toolsettings", "tube", [("unwrap", {})]),
    ("tube_unwrap_fill", "tube", [("unwrap", {**CONF, "fill_holes": True})]),
    ("tube_unwrap_seam", "tube_seam", [("unwrap", CONF)]),
    ("circle_ngon_unwrap", "circle_ngon", [("unwrap", CONF)]),
    ("grid_unwrap", "grid", [("unwrap", CONF)]),
    ("grid_smart_project", "grid", [("smart_project", {})]),
    ("monkey_smart_project", "monkey", [("smart_project", {})]),
    # Packer goldens: Blender's own smart-project layout of Suzanne (`monkey_smart_project`)
    # repacked with each shape / rotation / margin method.
    ("monkey_pack_default", "monkey", [("smart_project", {}), ("pack_islands", {})]),
    (
        "monkey_pack_aabb",
        "monkey",
        [("smart_project", {}), ("pack_islands", {"shape_method": "AABB"})],
    ),
    (
        "monkey_pack_convex",
        "monkey",
        [("smart_project", {}), ("pack_islands", {"shape_method": "CONVEX"})],
    ),
    (
        "monkey_pack_norotate",
        "monkey",
        [("smart_project", {}), ("pack_islands", {"rotate": False, "shape_method": "AABB"})],
    ),
    (
        "monkey_pack_cardinal",
        "monkey",
        [("smart_project", {}), ("pack_islands", {"rotate_method": "CARDINAL"})],
    ),
    (
        "monkey_pack_fraction",
        "monkey",
        [("smart_project", {}), ("pack_islands", {"margin_method": "FRACTION", "margin": 0.02})],
    ),
    (
        "monkey_pack_add",
        "monkey",
        [
            ("smart_project", {}),
            ("pack_islands", {"margin_method": "ADD", "margin": 0.01, "shape_method": "AABB"}),
        ],
    ),
    ("cyl8_cube_project", "cyl8", [("cube_project", {})]),
    (
        "cyl8_pack_merge_overlap",
        "cyl8",
        [("cube_project", {}), ("pack_islands", {"merge_overlap": True})],
    ),
]

# Unwraps with pinned UVs (`.pn.UVMap`): {vertex: uv} pinned on every corner of the vertex.
PINNED_CASES = [
    ("grid_unwrap_pin2", "grid", [("unwrap", CONF)], {0: [0.1, 0.2], 19: [0.9, 0.7]}),
    ("sphere_unwrap_pin2", "sphere_meridian", [("unwrap", CONF)], {0: [0.2, 0.1], 30: [0.6, 0.8]}),
    ("sphere_unwrap_pin1", "sphere_meridian", [("unwrap", CONF)], {30: [0.4, 0.3]}),
]


def run_case(setup, steps, pins=None):
    reset()
    build, seams = SETUPS[setup]
    build()
    ob = bpy.context.object
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_mode(type='FACE')
    bpy.ops.mesh.select_all(action='SELECT')
    if seams is not None:
        bm = bmesh.from_edit_mesh(ob.data)
        bm.edges.ensure_lookup_table()
        for i in seams(bm):
            bm.edges[i].seam = True
        bmesh.update_edit_mesh(ob.data)
    if pins:
        bm = bmesh.from_edit_mesh(ob.data)
        uv = bm.loops.layers.uv.active
        pin = bm.loops.layers.bool.get(".pn.UVMap") or bm.loops.layers.bool.new(".pn.UVMap")
        uv = bm.loops.layers.uv.active
        for f in bm.faces:
            for l in f.loops:
                if l.vert.index in pins:
                    l[uv].uv = pins[l.vert.index]
                    l[pin] = True
        bmesh.update_edit_mesh(ob.data)
    before = dump(ob)
    results = []
    for op, kw in steps:
        results.append(sorted(getattr(bpy.ops.uv, op)(**kw)))
    after = dump(ob)
    return {
        "setup": setup,
        "steps": [[op, kw] for op, kw in steps],
        "results": results,
        "before": before,
        "after_uvs": after["uvs"],
    }


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = argv[argv.index("--out") + 1]
    cases = {}
    setups = {}
    for name, setup, steps in CASES:
        case = run_case(setup, steps)
        # Cases share their setup's edit mesh (`setups[<setup>]`) when it is identical. It is
        # not always: a UV sphere built after other operators in the same session can come out
        # with another edge and face order (BMesh mempool reuse), so such a case keeps its own.
        if setups.setdefault(setup, case["before"]) == case["before"]:
            del case["before"]
        cases[name] = case
    for name, setup, steps, pins in PINNED_CASES:
        cases[name] = run_case(setup, steps, pins)
        cases[name]["pins"] = {str(k): v for k, v in pins.items()}
    with open(out, "w") as fh:
        json.dump(
            {"blender": bpy.app.version_string, "setups": setups, "cases": cases},
            fh,
            separators=(",", ":"),
        )
        fh.write("\n")


main()
