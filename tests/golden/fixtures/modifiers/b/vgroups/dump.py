# SPDX-License-Identifier: GPL-3.0-or-later
# WP 4.3: Blender 5.2.4 dumps of modifiers driven by vertex groups, for the unit tests of
# crates/frap-modifiers/src/types/*.rs (frap has no `vertex_groups.new` before M6, so these
# settings cannot be golden cases yet). Regenerate with:
#
#   Blender -b --factory-startup --python tests/golden/fixtures/modifiers/b/vgroups/dump.py
#
# Each `<type>.json` holds `{"blender": version, "cases": [{name, props, input, output}]}`:
# `input` is the mesh before the modifier (positions, edges, faces, vertex-group names and
# per-vertex `[group, weight]` pairs, sharp_face), `output` the evaluated mesh (positions,
# edges, faces, corner normals, and the attributes the modifier writes).
import json
import math
import os

import bpy

OUT = os.path.dirname(os.path.abspath(__file__))


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def mesh_dump(me, weights=True, attrs=()):
    d = {
        "positions": [list(v.co) for v in me.vertices],
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
    }
    if weights:
        d["groups"] = []
        d["weights"] = [[[g.group, g.weight] for g in v.groups] for v in me.vertices]
    sf = me.attributes.get("sharp_face")
    if sf is not None:
        d["sharp_face"] = [x.value for x in sf.data]
    d["corner_normals"] = [list(n.vector) for n in me.corner_normals]
    for name in attrs:
        a = me.attributes.get(name)
        if a is None:
            d[name] = None
            continue
        field = {"INT16_2D": "value", "BOOLEAN": "value", "FLOAT": "value",
                 "INT": "value", "FLOAT2": "vector", "FLOAT_VECTOR": "vector"}[a.data_type]
        d[name] = {"domain": a.domain, "data_type": a.data_type,
                   "data": [list(getattr(x, field)) if hasattr(getattr(x, field), "__len__")
                            else getattr(x, field) for x in a.data]}
    return d


def weight_fn(kind):
    # Deterministic weight patterns over the vertex position (some vertices unassigned).
    if kind == "x_ramp":
        return lambda i, co: max(0.0, min(1.0, (co.x + 1.0) * 0.5))
    if kind == "z_ramp":
        return lambda i, co: max(0.0, min(1.0, (co.z + 1.0) * 0.5))
    if kind == "half":
        return lambda i, co: (1.0 if co.x > 0.0 else None)
    if kind == "every_third":
        return lambda i, co: (None if i % 3 == 0 else 0.25 + 0.5 * ((i * 7) % 5) / 4.0)
    if kind == "none":
        return lambda i, co: None
    raise ValueError(kind)


def make(prim, groups):
    """`prim`: callable adding the active object; `groups`: [(name, kind)]."""
    reset()
    prim()
    ob = bpy.context.object
    for name, kind in groups:
        vg = ob.vertex_groups.new(name=name)
        fn = weight_fn(kind)
        for v in ob.data.vertices:
            w = fn(v.index, v.co)
            if w is not None:
                vg.add([v.index], w, 'REPLACE')
    return ob


def run(prim, groups, mtype, props, attrs=(), pre=None):
    ob = make(prim, groups)
    if pre is not None:
        pre(ob)
    inp = mesh_dump(ob.data, attrs=())
    inp["groups"] = [g.name for g in ob.vertex_groups]
    md = ob.modifiers.new(name="M", type=mtype)
    for k, v in props.items():
        setattr(md, k, v)
    dg = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(dg)
    me = ev.to_mesh()
    out = mesh_dump(me, weights=True, attrs=attrs)
    out["groups"] = [g.name for g in ob.vertex_groups]
    ev.to_mesh_clear()
    return {"props": props, "input": inp, "output": out}


def grid():
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)


def sphere():
    bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1)


def cube():
    bpy.ops.mesh.primitive_cube_add(size=2)


def monkey():
    bpy.ops.mesh.primitive_monkey_add(size=2)


def cylinder():
    bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=1, depth=2)


def write(name, cases):
    data = {"blender": bpy.app.version_string, "cases": cases}
    with open(os.path.join(OUT, name + ".json"), "w") as f:
        json.dump(data, f, indent=None, separators=(",", ":"))
        f.write("\n")


def smooth_cases():
    cases = []
    for inv in (False, True):
        c = run(sphere, [("G", "z_ramp")], 'SMOOTH',
                {"factor": 0.8, "iterations": 3, "vertex_group": "G",
                 "invert_vertex_group": inv, "use_x": True, "use_y": True, "use_z": True})
        c["name"] = "sphere_z_ramp_inv" if inv else "sphere_z_ramp"
        cases.append(c)
    c = run(grid, [("G", "every_third")], 'SMOOTH',
            {"factor": 1.5, "iterations": 2, "vertex_group": "G",
             "invert_vertex_group": False, "use_x": True, "use_y": False, "use_z": True})
    c["name"] = "grid_unassigned"
    cases.append(c)
    write("smooth", cases)


def cast_cases():
    cases = []
    base = {"factor": 0.7, "radius": 0.0, "size": 0.0, "use_radius_as_size": False,
            "use_x": True, "use_y": True, "use_z": True, "vertex_group": "G",
            "invert_vertex_group": False}
    for name, prim, kind, extra in [
        ("sphere_cube_half", cube, "half", {"cast_type": 'SPHERE'}),
        ("cylinder_monkey_xramp", monkey, "x_ramp", {"cast_type": 'CYLINDER', "radius": 0.9}),
        ("cuboid_sphere_inv", sphere, "z_ramp",
         {"cast_type": 'CUBOID', "invert_vertex_group": True, "factor": 1.2}),
        ("cuboid_monkey_size", monkey, "every_third",
         {"cast_type": 'CUBOID', "size": 0.5, "use_y": False}),
    ]:
        props = dict(base)
        props.update(extra)
        c = run(prim, [("G", kind)], 'CAST', props)
        c["name"] = name
        cases.append(c)
    write("cast", cases)


def simple_deform_cases():
    cases = []
    base = {"deform_method": 'TWIST', "deform_axis": 'Z', "vertex_group": "G",
            "invert_vertex_group": False, "factor": 0.785398, "limits": (0.0, 1.0),
            "lock_x": False, "lock_y": False, "lock_z": False}
    for name, prim, groups, extra in [
        ("twist_cylinder_zramp", cylinder, [("G", "z_ramp")], {"factor": 1.3}),
        ("bend_grid_xramp", grid, [("G", "x_ramp")],
         {"deform_method": 'BEND', "deform_axis": 'Y', "factor": 2.0, "limits": (0.2, 0.8)}),
        ("taper_monkey_inv", monkey, [("G", "every_third")],
         {"deform_method": 'TAPER', "deform_axis": 'X', "factor": 0.6,
          "invert_vertex_group": True, "lock_y": True}),
        ("stretch_sphere_lock", sphere, [("G", "half")],
         {"deform_method": 'STRETCH', "deform_axis": 'Y', "factor": -0.4,
          "lock_x": True, "lock_z": True, "limits": (0.1, 0.7)}),
        ("missing_group", sphere, [("Other", "half")], {"factor": 0.9}),
        ("empty_group", sphere, [("E", "none")],
         {"vertex_group": "E", "factor": 0.9}),
        ("empty_group_inv", sphere, [("E", "none")],
         {"vertex_group": "E", "factor": 0.9, "invert_vertex_group": True}),
    ]:
        props = dict(base)
        props.update(extra)
        c = run(prim, groups, 'SIMPLE_DEFORM', props)
        c["name"] = name
        cases.append(c)
    write("simple_deform", cases)


def laplaciansmooth_cases():
    cases = []
    base = {"use_x": True, "use_y": True, "use_z": True, "use_volume_preserve": True,
            "use_normalized": True, "lambda_factor": 0.01, "lambda_border": 0.01,
            "iterations": 1, "vertex_group": "", "invert_vertex_group": False}
    for name, prim, groups, extra in [
        ("defaults_monkey", monkey, [], {}),
        ("sphere_strong", sphere, [], {"lambda_factor": 2.0, "iterations": 3}),
        ("grid_border_neg", grid, [("G", "x_ramp")],
         {"lambda_factor": 1.5, "lambda_border": -0.8, "use_normalized": False,
          "vertex_group": "G", "iterations": 2, "use_volume_preserve": False}),
        ("monkey_vg_inv", monkey, [("G", "z_ramp")],
         {"lambda_factor": 3.0, "lambda_border": 1.0, "vertex_group": "G",
          "invert_vertex_group": True, "use_y": False}),
        ("cube_area_weighted", cube, [], {"lambda_factor": 0.5, "use_normalized": False}),
    ]:
        props = dict(base)
        props.update(extra)
        c = run(prim, groups, 'LAPLACIANSMOOTH', props)
        c["name"] = name
        cases.append(c)
    write("laplaciansmooth", cases)


ALL = [smooth_cases, cast_cases, simple_deform_cases, laplaciansmooth_cases]

if __name__ == "__main__":
    for f in ALL:
        f()
