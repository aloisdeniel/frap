# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump the meshes Blender 5.2 builds for every `mesh.primitive_*_add` case, plus mesh-level
reference cases (normals, loop triangles, `from_pydata`), to JSON (ARCHITECTURE §6.1, WP 1.4).

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_primitives.py -- \
        --out spec/blender-5.2/primitives [--filter <substring>]

Writes one `<case>.json` per case and `_meta.json` (Blender version and build hash). Each file
holds the operator idname and keyword arguments (primitive cases), the construction recipe
(normals cases) or the `from_pydata` input (pydata cases), the created object's transform, and
the mesh: positions, edges, faces (corner vertices), corner edges, every attribute in
`mesh.attributes` order (name, domain, data type, values; the four topology attributes have
`data: null` because they are written out above), UV layer flags, `normals_domain`,
vertex/face/corner normals and loop triangles (corner indices).

With `--sweep` it writes a larger parameter sweep (`sweep__*.json`, primitive cases only) that the
live test `sweep_matches_live_blender` compares without committing it.

Findings recorded while writing it (Blender 5.2.4): datablock names come from an empty factory
scene; `mesh.attributes` lists attributes in creation order; the UV sphere's edge and face order
changes between runs of the same build (pointer-keyed `GHash` in the extrusion), its vertex order
and positions do not.

Floats are written as the Python `repr` of the stored f32 value, so parsing them as f64 and
narrowing to f32 is exact. frap's tests (`crates/frap-mesh/tests/primitives_vs_blender.rs`,
`crates/frap-mesh/tests/mesh_vs_blender.rs`) read these files; the live test
`dump_matches_live_blender` re-runs this script when Blender is available and diffs the output
against the committed copies.
"""

import json
import math
import os
import sys

import bpy

# --------------------------------------------------------------------------- primitive cases

# (case name, operator, keyword arguments). Every primitive has its defaults and at least two
# variants that exercise each generator parameter (ARCHITECTURE §22.3, WP 1.4).
PRIMITIVE_CASES = [
    ("plane__default", "primitive_plane_add", {}),
    ("plane__size3_5_nouv", "primitive_plane_add", {"size": 3.5, "calc_uvs": False}),
    ("plane__size0_25_scale", "primitive_plane_add", {"size": 0.25, "scale": (2.0, 1.0, 0.5)}),
    ("cube__default", "primitive_cube_add", {}),
    ("cube__size3_nouv", "primitive_cube_add", {"size": 3.0, "calc_uvs": False}),
    ("cube__size1_scale123", "primitive_cube_add", {"size": 1.0, "scale": (1.0, 2.0, 3.0)}),
    ("circle__default", "primitive_circle_add", {}),
    ("circle__v5_r2_ngon", "primitive_circle_add",
     {"vertices": 5, "radius": 2.0, "fill_type": "NGON"}),
    ("circle__v7_r0_5_trifan", "primitive_circle_add",
     {"vertices": 7, "radius": 0.5, "fill_type": "TRIFAN"}),
    ("circle__v3_ngon_nouv", "primitive_circle_add",
     {"vertices": 3, "fill_type": "NGON", "calc_uvs": False}),
    ("uv_sphere__default", "primitive_uv_sphere_add", {}),
    ("uv_sphere__s5_r3_rad0_5", "primitive_uv_sphere_add",
     {"segments": 5, "ring_count": 3, "radius": 0.5}),
    ("uv_sphere__s12_r7_rad2_scale_nouv", "primitive_uv_sphere_add",
     {"segments": 12, "ring_count": 7, "radius": 2.0, "scale": (1.0, 1.0, 2.0), "calc_uvs": False}),
    ("uv_sphere__s3_r4", "primitive_uv_sphere_add", {"segments": 3, "ring_count": 4}),
    ("uv_sphere__s7_r5", "primitive_uv_sphere_add", {"segments": 7, "ring_count": 5}),
    ("ico_sphere__default", "primitive_ico_sphere_add", {}),
    ("ico_sphere__sub1_rad2", "primitive_ico_sphere_add", {"subdivisions": 1, "radius": 2.0}),
    ("ico_sphere__sub3_rad0_5", "primitive_ico_sphere_add", {"subdivisions": 3, "radius": 0.5}),
    ("ico_sphere__sub4_scale_nouv", "primitive_ico_sphere_add",
     {"subdivisions": 4, "scale": (2.0, 1.0, 1.0), "calc_uvs": False}),
    ("cylinder__default", "primitive_cylinder_add", {}),
    ("cylinder__v6_d3_trifan", "primitive_cylinder_add",
     {"vertices": 6, "depth": 3.0, "end_fill_type": "TRIFAN"}),
    ("cylinder__v5_r2_nothing_scale", "primitive_cylinder_add",
     {"vertices": 5, "radius": 2.0, "end_fill_type": "NOTHING", "scale": (1.0, 1.0, 0.5)}),
    ("cylinder__v3_nouv", "primitive_cylinder_add", {"vertices": 3, "calc_uvs": False}),
    ("cone__default", "primitive_cone_add", {}),
    ("cone__v6_r1_0_5_trifan", "primitive_cone_add",
     {"vertices": 6, "radius1": 1.0, "radius2": 0.5, "end_fill_type": "TRIFAN"}),
    ("cone__v8_r0_1_nothing", "primitive_cone_add",
     {"vertices": 8, "radius1": 0.0, "radius2": 1.0, "end_fill_type": "NOTHING"}),
    ("cone__v5_r2_1_d0_5_ngon_nouv", "primitive_cone_add",
     {"vertices": 5, "radius1": 2.0, "radius2": 1.0, "depth": 0.5, "calc_uvs": False}),
    ("cone__v4_r0_0", "primitive_cone_add", {"vertices": 4, "radius1": 0.0, "radius2": 0.0}),
    ("grid__default", "primitive_grid_add", {}),
    ("grid__x3_y5_size4", "primitive_grid_add",
     {"x_subdivisions": 3, "y_subdivisions": 5, "size": 4.0}),
    ("grid__x1_y1_nouv", "primitive_grid_add",
     {"x_subdivisions": 1, "y_subdivisions": 1, "calc_uvs": False}),
    ("grid__x7_y2_size0_5", "primitive_grid_add",
     {"x_subdivisions": 7, "y_subdivisions": 2, "size": 0.5}),
    ("monkey__default", "primitive_monkey_add", {}),
    ("monkey__size4_nouv", "primitive_monkey_add", {"size": 4.0, "calc_uvs": False}),
    ("monkey__size0_5", "primitive_monkey_add", {"size": 0.5}),
    ("torus__default", "primitive_torus_add", {}),
    ("torus__maj12_min6_r2_0_5", "primitive_torus_add",
     {"major_segments": 12, "minor_segments": 6, "major_radius": 2.0, "minor_radius": 0.5}),
    ("torus__ext_int_nouv", "primitive_torus_add",
     {"mode": "EXT_INT", "abso_major_rad": 1.5, "abso_minor_rad": 0.75, "generate_uvs": False}),
    ("torus__maj3_min3", "primitive_torus_add", {"major_segments": 3, "minor_segments": 3}),
]

# --------------------------------------------------------------------------- normals cases


def _sharp_edges(me, indices):
    attr = me.attributes.get("sharp_edge") or me.attributes.new("sharp_edge", 'BOOLEAN', 'EDGE')
    for i in indices:
        attr.data[i].value = True


def _smooth_uv_sphere():
    bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=5)
    bpy.context.object.data.shade_smooth()


def _smooth_cube_sharp_edges():
    bpy.ops.mesh.primitive_cube_add()
    me = bpy.context.object.data
    me.shade_smooth()
    _sharp_edges(me, [0, 5, 9])


def _cube_mixed_sharp_faces():
    bpy.ops.mesh.primitive_cube_add()
    me = bpy.context.object.data
    for i, v in enumerate([True, False, False, True, False, False]):
        me.attributes["sharp_face"].data[i].value = v


def _smooth_cylinder():
    bpy.ops.mesh.primitive_cylinder_add(vertices=7)
    bpy.context.object.data.shade_smooth()


def _smooth_torus_sharp_edges():
    bpy.ops.mesh.primitive_torus_add(major_segments=10, minor_segments=5)
    me = bpy.context.object.data
    me.shade_smooth()
    _sharp_edges(me, list(range(0, len(me.edges), 7)))


def _smooth_monkey():
    bpy.ops.mesh.primitive_monkey_add()
    bpy.context.object.data.shade_smooth()


# The frap test rebuilds each mesh from the dumped topology and attributes and compares the
# derived normals, `normals_domain` and loop triangles.
MESH_CASES = [
    ("mesh__smooth_uv_sphere", _smooth_uv_sphere,
     "primitive_uv_sphere_add(segments=8, ring_count=5); shade_smooth()"),
    ("mesh__smooth_cube_sharp_edges", _smooth_cube_sharp_edges,
     "primitive_cube_add(); shade_smooth(); sharp_edge[0,5,9]=True"),
    ("mesh__cube_mixed_sharp_faces", _cube_mixed_sharp_faces,
     "primitive_cube_add(); sharp_face=[T,F,F,T,F,F]"),
    ("mesh__smooth_cylinder", _smooth_cylinder,
     "primitive_cylinder_add(vertices=7); shade_smooth()"),
    ("mesh__smooth_torus_sharp_edges", _smooth_torus_sharp_edges,
     "primitive_torus_add(major_segments=10, minor_segments=5); shade_smooth(); "
     "sharp_edge[0::7]=True"),
    ("mesh__smooth_monkey", _smooth_monkey, "primitive_monkey_add(); shade_smooth()"),
]

# --------------------------------------------------------------------------- from_pydata cases


def _star_verts():
    verts = []
    for i in range(14):
        r = 1.0 if i % 2 == 0 else 0.35
        a = 2.0 * math.pi * i / 14.0
        verts.append((r * math.cos(a), r * math.sin(a), 0.05 * (i % 3)))
    return verts


def _big_grid(n=40):
    verts = [(float(x), float(y), 0.0) for y in range(n) for x in range(n)]
    faces = []
    for y in range(n - 1):
        for x in range(n - 1):
            a = y * n + x
            faces.append((a, a + 1, a + n + 1, a + n))
    return verts, faces


_BIG_VERTS, _BIG_FACES = _big_grid()

# `Mesh.from_pydata(vertices, edges, faces, shade_flat)`; the inputs are written into the case
# file ("input") so frap's Rust `Mesh::from_pydata` test rebuilds the same mesh.
PYDATA_CASES = [
    # Non-manifold fan: three quads share edge (0,1); a triangle hangs off a boundary edge.
    ("mesh__nonmanifold_smooth",
     [(0, 0, 0), (0, 0, 1), (1, 0, 0), (1, 0, 1), (0, 1, 0), (0, 1, 1), (-1, -1, 0), (-1, -1, 1),
      (2, 1, 0)],
     [], [(0, 1, 3, 2), (1, 0, 4, 5), (0, 1, 7, 6), (2, 3, 8)], False),
    # Concave and non-planar n-gons (BLI_polyfill_2d without beautify for loop triangles).
    ("mesh__ngons",
     [(0, 0, 0), (4, 0, 0), (4, 3, 0), (3, 3, 0), (3, 1, 0), (1, 1, 0), (1, 3, 0), (0, 3, 0.2),
      (6, 0, 0), (7, 2, 0.5), (6, 4, 0), (5, 2, 0.1), (8, 3, 1)],
     [], [(0, 1, 2, 3, 4, 5, 6, 7), (1, 8, 9, 10, 11), (8, 12, 9)], False),
    ("mesh__star_ngon", _star_verts(), [], [tuple(range(14))], True),
    # Arrowhead quad (reflex corner 3): corners 1 and 3 lie on the same side of the 0-2
    # diagonal, so loop triangles use the 1-3 split; plus a planar and a non-planar quad.
    ("mesh__quad_split_rule",
     [(0, 0, 0), (1, 1, 0), (2, 0, 0), (1, 0.5, 0), (3, 0, 0), (4, 0, 0.7), (4, 1, 0), (3, 1, 0.4)],
     [], [(0, 1, 2, 3), (4, 5, 6, 7), (2, 4, 7, 1)], False),
    # Opposite winding on neighbouring faces: the shared edge splits the smooth fan.
    ("mesh__flipped_neighbour_smooth",
     [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 0, 0.3), (2, 1, 0.3)],
     [], [(0, 1, 2, 3), (1, 2, 5, 4)], False),
    ("pydata__edges_and_faces",
     [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 0, 0), (2, 1, 0), (3, 3, 3)],
     [(5, 4), (0, 1), (6, 2)], [(0, 1, 2, 3), (1, 4, 5, 2)], True),
    ("pydata__duplicate_edges",
     [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0)],
     [(1, 0), (0, 1), (2, 3)], [(0, 1, 2, 3)], True),
    ("pydata__loose_edges_only", [(0, 0, 0), (1, 0, 0), (1, 1, 0)], [(0, 1), (2, 1)], [], True),
    ("pydata__verts_only", [(0, 0, 0), (1, 2, 3)], [], [], True),
    # >= 1000 faces: Blender switches mesh_calc_edges to several hash maps (edge order changes).
    ("pydata__big_grid", _BIG_VERTS, [], _BIG_FACES, False),
]

# --------------------------------------------------------------------------- serialisation

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
    if t in ('FLOAT_COLOR', 'BYTE_COLOR'):
        return [list(x.color) for x in d]
    if t in ('INT16_2D', 'INT32_2D'):
        return [list(x.value) for x in d]
    return None


def mesh_dump(me):
    me.calc_loop_triangles()
    return {
        "name": me.name,
        "positions": [list(v.co) for v in me.vertices],
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
        "corner_edges": [l.edge_index for l in me.loops],
        "attributes": [
            {"name": a.name, "domain": a.domain, "data_type": a.data_type, "data": _vals(a)}
            for a in me.attributes
        ],
        "uv_layers": [{"name": l.name, "active": l.active, "active_render": l.active_render}
                      for l in me.uv_layers],
        "normals_domain": me.normals_domain,
        "vertex_normals": [list(v.vector) for v in me.vertex_normals],
        "face_normals": [list(v.vector) for v in me.polygon_normals],
        "corner_normals": [list(v.vector) for v in me.corner_normals],
        "loop_triangles": [list(t.loops) for t in me.loop_triangles],
        "loop_triangle_polygons": [t.polygon_index for t in me.loop_triangles],
    }


def obj_dump(o):
    return {
        "name": o.name,
        "data": o.data.name if o.data else None,
        "location": list(o.location),
        "rotation_euler": list(o.rotation_euler),
        "scale": list(o.scale),
    }


def _fmt(value, indent=0):
    """JSON with dicts indented and every list of scalars on one line (diff-friendly, compact)."""
    pad = " " * indent
    if isinstance(value, dict):
        if not value:
            return "{}"
        items = [f'{pad} {json.dumps(k)}: {_fmt(v, indent + 1)}' for k, v in value.items()]
        return "{\n" + ",\n".join(items) + "\n" + pad + "}"
    if isinstance(value, (list, tuple)):
        if any(isinstance(v, dict) for v in value):
            items = [pad + " " + _fmt(v, indent + 1) for v in value]
            return "[\n" + ",\n".join(items) + "\n" + pad + "]"
        if any(isinstance(v, (list, tuple)) for v in value):
            items = [pad + " " + json.dumps(v, separators=(",", ":")) for v in value]
            return "[\n" + ",\n".join(items) + "\n" + pad + "]"
        return json.dumps(list(value), separators=(",", ":"))
    return json.dumps(value)


def write(out_dir, name, data):
    path = os.path.join(out_dir, name + ".json")
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(_fmt(data))
        fh.write("\n")


def reset():
    # Empty factory scene: datablock names are the operators' base names ("Cube", not
    # "Cube.001" next to the startup cube).
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.object.select_all(action='DESELECT')


def sweep_cases():
    """Parameter sweep for the live test (`--sweep`, not committed): every fill type and radius
    combination of circle/cylinder/cone for 3..13 segments, ico spheres up to 5 subdivisions,
    UV spheres, grids and tori of several sizes."""
    out = []
    fills = ("NOTHING", "NGON", "TRIFAN")
    for n in range(3, 14):
        for fill in fills:
            out.append((f"sweep__circle_{fill}_{n}", "primitive_circle_add",
                        {"vertices": n, "fill_type": fill, "radius": 1.5}))
            out.append((f"sweep__cylinder_{fill}_{n}", "primitive_cylinder_add",
                        {"vertices": n, "end_fill_type": fill, "depth": 1.5}))
            for r1, r2 in ((1.0, 0.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.5), (0.25, 1.0)):
                out.append((f"sweep__cone_{fill}_{n}_{r1}_{r2}", "primitive_cone_add",
                            {"vertices": n, "end_fill_type": fill, "radius1": r1, "radius2": r2}))
    for s in range(1, 6):
        out.append((f"sweep__ico_{s}", "primitive_ico_sphere_add", {"subdivisions": s, "radius": 1.3}))
    for seg, ring in ((3, 3), (4, 3), (5, 5), (6, 4), (8, 6), (13, 7), (16, 8), (24, 12), (48, 24),
                      (64, 32)):
        out.append((f"sweep__uv_sphere_{seg}_{ring}", "primitive_uv_sphere_add",
                    {"segments": seg, "ring_count": ring, "radius": 0.75}))
    for x, y in ((1, 1), (2, 9), (17, 3), (40, 40)):
        out.append((f"sweep__grid_{x}_{y}", "primitive_grid_add",
                    {"x_subdivisions": x, "y_subdivisions": y, "size": 3.0}))
    for maj, mnr in ((3, 3), (7, 4), (100, 12)):
        out.append((f"sweep__torus_{maj}_{mnr}", "primitive_torus_add",
                    {"major_segments": maj, "minor_segments": mnr}))
    return out


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    args = {"out": None, "filter": "", "sweep": False}
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            args["out"] = argv[i + 1]
            i += 2
        elif argv[i] == "--filter":
            args["filter"] = argv[i + 1]
            i += 2
        elif argv[i] == "--sweep":
            args["sweep"] = True
            i += 1
        else:
            raise SystemExit(f"dump_primitives.py: unknown argument {argv[i]!r}")
    if not args["out"]:
        raise SystemExit("dump_primitives.py: --out DIR is required")
    return args


def main():
    args = parse_args()
    out_dir = args["out"]
    os.makedirs(out_dir, exist_ok=True)
    write(out_dir, "_meta", {
        "blender_version": bpy.app.version_string,
        "build_hash": bpy.app.build_hash.decode(),
        "factory_startup": True,
        "script": "tools/blender/dump_primitives.py",
    })
    cases = sweep_cases() if args["sweep"] else PRIMITIVE_CASES
    for name, op, kwargs in cases:
        if args["filter"] not in name:
            continue
        reset()
        getattr(bpy.ops.mesh, op)(**kwargs)
        ob = bpy.context.object
        write(out_dir, name, {
            "op": "mesh." + op,
            "kwargs": {k: list(v) if isinstance(v, tuple) else v for k, v in kwargs.items()},
            "object": obj_dump(ob),
            "mesh": mesh_dump(ob.data),
        })
    if args["sweep"]:
        return
    for name, build, recipe in MESH_CASES:
        if args["filter"] not in name:
            continue
        reset()
        build()
        write(out_dir, name, {"recipe": recipe, "mesh": mesh_dump(bpy.context.object.data)})
    for name, verts, edges, faces, shade_flat in PYDATA_CASES:
        if args["filter"] not in name:
            continue
        reset()
        me = bpy.data.meshes.new(name)
        me.from_pydata(verts, edges, faces, shade_flat=shade_flat)
        write(out_dir, name, {
            "input": {"vertices": [[float(c) for c in v] for v in verts],
                      "edges": [list(e) for e in edges],
                      "faces": [list(f) for f in faces],
                      "shade_flat": shade_flat},
            "mesh": mesh_dump(me),
        })


if __name__ == "__main__":
    main()
