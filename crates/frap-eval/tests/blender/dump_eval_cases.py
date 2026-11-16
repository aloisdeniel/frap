# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump Blender 5.2's evaluated transforms and bounds for the WP 1.13 reference cases
(ARCHITECTURE §7.3): world matrices with parents and `matrix_parent_inverse` (OBJECT, VERTEX,
VERTEX_3 parent types, every rotation mode, delta channels, negative scale, unnormalized and zero
quaternions), `dimensions` and `bound_box`.

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup --python crates/frap-eval/tests/blender/dump_eval_cases.py -- \
        --out crates/frap-eval/tests/data/eval_cases.json

The output lists meshes (`positions`, `faces`) and objects in creation order (parents first).
Each object records its inputs (the transform channels, `parent`, `parent_type`,
`parent_vertices`, `matrix_parent_inverse` row-major, `data`) and the outputs after
`view_layer.update()`: `matrix_world` (row-major), `dimensions`, `bound_box`.

Floats are written as the Python `repr` of the stored f32 value, so parsing them as f64 and
narrowing to f32 is exact. `crates/frap-eval/tests/vs_blender.rs` reads the file; its live test
re-runs this script when Blender is available and diffs the output against the committed copy.
"""

import json
import sys

import bpy
from mathutils import Matrix


def args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = None
    for i, a in enumerate(argv):
        if a == "--out":
            out = argv[i + 1]
    if out is None:
        raise SystemExit("usage: ... -- --out <file.json>")
    return out


def rows(m):
    return [[float(v) for v in r] for r in m]


D = bpy.data
SCENE = bpy.context.scene

# An empty scene of our own: the factory objects play no part.
for ob in list(D.objects):
    D.objects.remove(ob)

MESHES = []
OBJECTS = []


def mesh(name, verts, faces):
    me = D.meshes.new(name)
    me.from_pydata(verts, [], faces)
    me.update()
    MESHES.append(me)
    return me


def obj(name, data=None, parent=None, parent_type="OBJECT", parent_vertices=(0, 0, 0),
        parent_inverse=None, rotation_mode="XYZ", **channels):
    ob = D.objects.new(name, data)
    SCENE.collection.objects.link(ob)
    ob.rotation_mode = rotation_mode
    for k, v in channels.items():
        setattr(ob, k, v)
    if parent is not None:
        ob.parent = D.objects[parent]
        ob.parent_type = parent_type
        if parent_type in ("VERTEX", "VERTEX_3"):
            ob.parent_vertices = parent_vertices
    if parent_inverse is not None:
        ob.matrix_parent_inverse = Matrix(parent_inverse)
    OBJECTS.append(ob)
    return ob


# A deliberately irregular mesh (vertex parents, bounds).
lump = mesh("Lump", [
    (-1.25, -0.5, -0.75), (1.5, -0.625, -0.25), (1.125, 0.875, -0.5), (-0.875, 1.0, -0.375),
    (-1.0, -0.75, 0.875), (1.375, -0.5, 1.25), (0.75, 1.125, 1.0), (-1.125, 0.625, 1.5),
], [(0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1), (1, 5, 6, 2), (2, 6, 7, 3), (3, 7, 4, 0)])
flat = mesh("Flat", [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.0, 3.0, 0.0)], [(0, 1, 2)])
empty_mesh = mesh("Nothing", [], [])

SHEAR = [[1.0, 0.25, 0.0, 0.5], [0.0, 1.0, -0.125, 1.0], [0.375, 0.0, 1.0, -2.0],
         [0.0, 0.0, 0.0, 1.0]]
SCALED = [[0.5, 0.0, 0.0, 0.0], [0.0, 2.0, 0.0, 0.0], [0.0, 0.0, -1.5, 0.0],
          [0.0, 0.0, 0.0, 1.0]]

obj("RootXYZ", location=(1.0, -2.0, 0.5), rotation_euler=(0.3, -1.2, 2.5), scale=(1.5, 0.75, 2.0))
obj("RootZXY", rotation_mode="ZXY", location=(-0.25, 3.0, 1.0),
    rotation_euler=(-0.7, 0.4, 1.9), scale=(-1.0, 2.0, 0.5),
    delta_location=(0.125, 0.25, -0.5), delta_rotation_euler=(0.2, 0.1, -0.3),
    delta_scale=(1.0, -0.5, 2.0))
obj("RootQuat", rotation_mode="QUATERNION", location=(0.0, 0.0, 4.0),
    rotation_quaternion=(2.0, 0.5, -1.0, 0.25), scale=(1.0, 1.0, 3.0),
    delta_rotation_quaternion=(0.9, 0.1, 0.2, -0.3))
obj("RootQuatZero", rotation_mode="QUATERNION", rotation_quaternion=(0.0, 0.0, 0.0, 0.0))
obj("RootAxis", rotation_mode="AXIS_ANGLE", rotation_axis_angle=(1.1, 0.3, -2.0, 0.7),
    location=(5.0, 0.0, -1.0), scale=(0.5, 0.5, 0.5))
obj("RootAxisZero", rotation_mode="AXIS_ANGLE", rotation_axis_angle=(0.8, 0.0, 0.0, 0.0))
obj("ChildPlain", parent="RootXYZ", location=(0.5, 0.25, -1.0), rotation_euler=(0.1, 0.2, 0.3))
obj("ChildInverse", parent="RootQuat", parent_inverse=SHEAR, rotation_mode="YXZ",
    location=(-1.0, 2.0, 0.75), rotation_euler=(1.0, -0.5, 0.25), scale=(2.0, 1.0, 0.5))
obj("Grandchild", parent="ChildInverse", parent_inverse=SCALED, location=(0.3, 0.2, 0.1),
    rotation_euler=(0.0, 0.6, -0.4))
obj("GreatGrandchild", data=lump, parent="Grandchild", location=(1.0, 1.0, 1.0),
    scale=(1.0, -2.0, 0.5))
obj("LumpObject", data=lump, location=(2.0, -1.0, 0.0), rotation_euler=(0.4, 0.0, 1.3),
    scale=(1.25, 0.5, -0.75))
obj("VertexChild", parent="LumpObject", parent_type="VERTEX", parent_vertices=(5, 0, 0),
    location=(0.25, 0.5, 0.75), rotation_euler=(0.3, 0.3, 0.3))
obj("VertexChildInverse", parent="LumpObject", parent_type="VERTEX", parent_vertices=(2, 0, 0),
    parent_inverse=SHEAR, location=(1.0, 0.0, 0.0))
obj("VertexOutOfRange", parent="LumpObject", parent_type="VERTEX", parent_vertices=(99, 0, 0),
    location=(0.5, 0.5, 0.5))
obj("Vertex3Child", parent="LumpObject", parent_type="VERTEX_3", parent_vertices=(1, 6, 3),
    location=(0.1, -0.2, 0.3), rotation_euler=(0.5, -0.25, 0.125))
obj("Vertex3Inverse", parent="LumpObject", parent_type="VERTEX_3", parent_vertices=(0, 2, 7),
    parent_inverse=SCALED, scale=(1.0, 2.0, 3.0))
obj("FlatObject", data=flat, rotation_euler=(0.0, 0.0, 0.5))
obj("Vertex3Flat", parent="FlatObject", parent_type="VERTEX_3", parent_vertices=(0, 1, 2),
    location=(0.0, 0.0, 1.0))
obj("Vertex3Degenerate", parent="FlatObject", parent_type="VERTEX_3", parent_vertices=(0, 0, 0))
obj("NothingObject", data=empty_mesh, scale=(2.0, 2.0, 2.0))
obj("EmptyChildOfMesh", parent="GreatGrandchild", location=(0.0, 1.0, 0.0))

bpy.context.view_layer.update()


def f32s(v):
    return [float(x) for x in v]


out = {
    "blender": bpy.app.version_string,
    "meshes": [{
        "name": me.name,
        "positions": [f32s(v.co) for v in me.vertices],
        "faces": [list(p.vertices) for p in me.polygons],
    } for me in MESHES],
    "objects": [{
        "name": ob.name,
        "data": ob.data.name if ob.data else None,
        "location": f32s(ob.location),
        "rotation_mode": ob.rotation_mode,
        "rotation_euler": f32s(ob.rotation_euler),
        "rotation_quaternion": f32s(ob.rotation_quaternion),
        "rotation_axis_angle": f32s(ob.rotation_axis_angle),
        "scale": f32s(ob.scale),
        "delta_location": f32s(ob.delta_location),
        "delta_rotation_euler": f32s(ob.delta_rotation_euler),
        "delta_rotation_quaternion": f32s(ob.delta_rotation_quaternion),
        "delta_scale": f32s(ob.delta_scale),
        "parent": ob.parent.name if ob.parent else None,
        "parent_type": ob.parent_type,
        "parent_vertices": list(ob.parent_vertices),
        "matrix_parent_inverse": rows(ob.matrix_parent_inverse),
        "matrix_world": rows(ob.matrix_world),
        "dimensions": f32s(ob.dimensions),
        "bound_box": [f32s(v) for v in ob.bound_box],
    } for ob in OBJECTS],
}

with open(args(), "w", encoding="utf-8") as f:
    json.dump(out, f, indent=1)
    f.write("\n")
