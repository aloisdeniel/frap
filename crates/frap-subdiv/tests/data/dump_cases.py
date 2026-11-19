# SPDX-License-Identifier: GPL-3.0-or-later
# WP 4.4: writes the Subdivision Surface reference dumps of this directory with Blender 5.2.4:
# every case builds a mesh, adds a SUBSURF modifier and dumps the base mesh, the settings and
# the evaluated mesh (`to_mesh`) as JSON, read by `tests/blender_dumps.rs`.
# Usage: Blender -b --factory-startup --python dump_cases.py -- OUTDIR [filter]
import bpy, bmesh, json, sys, os

argv = sys.argv[sys.argv.index("--") + 1:]
OUT = argv[0]
FILT = argv[1] if len(argv) > 1 else None
os.makedirs(OUT, exist_ok=True)


def r(x):
    return float("%.7g" % x)


def mesh_dict(me, with_uv=True, attrs=()):
    me.calc_loop_triangles()
    d = {
        "positions": [[r(c) for c in v.co] for v in me.vertices],
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
        "corner_edges": [l.edge_index for l in me.loops],
    }
    if with_uv:
        d["uvs"] = [[[r(c) for c in l.uv] for l in uv.data] for uv in me.uv_layers]
    for a in attrs:
        if a in me.attributes:
            at = me.attributes[a]
            d[a] = [r(x.value) for x in at.data]
    return d


def clear():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob)
    for me in list(bpy.data.meshes):
        bpy.data.meshes.remove(me)


def run(name, build, settings, edit=None):
    if FILT and FILT not in name:
        return
    clear()
    build()
    ob = bpy.context.view_layer.objects.active
    if edit:
        edit(ob)
    me = ob.data
    inp = mesh_dict(me, attrs=("crease_edge", "crease_vert"))
    md = ob.modifiers.new("Subdivision", "SUBSURF")
    for k, v in settings.items():
        setattr(md, k, v)
    dg = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(dg)
    em = ev.to_mesh()
    out = mesh_dict(em, attrs=("crease_edge", "crease_vert"))
    ev.to_mesh_clear()
    s = {k: getattr(md, k) for k in ("levels", "quality", "use_limit_surface", "subdivision_type",
                                    "boundary_smooth", "uv_smooth", "use_creases")}
    with open(os.path.join(OUT, name + ".json"), "w") as f:
        json.dump({"name": name, "settings": s, "input": inp, "output": out}, f, separators=(",", ":"))
    print("wrote", name, len(out["positions"]))


def cube():
    bpy.ops.mesh.primitive_cube_add()


def creases(edge_vals, vert_vals=None):
    def f(ob):
        me = ob.data
        a = me.attributes.new("crease_edge", "FLOAT", "EDGE")
        for i, v in edge_vals.items():
            a.data[i].value = v
        if vert_vals:
            b = me.attributes.new("crease_vert", "FLOAT", "POINT")
            for i, v in vert_vals.items():
                b.data[i].value = v
    return f


for lv in (1, 2, 3):
    run("cube_l%d" % lv, cube, {"levels": lv})
run("cube_l2_nolimit", cube, {"levels": 2, "use_limit_surface": False})
run("cube_l1_simple", cube, {"levels": 1, "subdivision_type": "SIMPLE"})
run("cube_l2_simple", cube, {"levels": 2, "subdivision_type": "SIMPLE"})
run("cube_l2_simple_nolimit", cube, {"levels": 2, "subdivision_type": "SIMPLE", "use_limit_surface": False})
run("cube_l2_q1", cube, {"levels": 2, "quality": 1})
run("cube_l3_q2", cube, {"levels": 3, "quality": 2})
run("cube_l4", cube, {"levels": 4})
run("cube_l2_crease", cube, {"levels": 2}, creases({0: 1.0, 1: 1.0, 2: 1.0, 3: 1.0, 5: 0.5, 7: 0.3}))
run("cube_l2_crease_strong", cube, {"levels": 2}, creases({0: 0.8, 4: 0.9}))
run("cube_l2_crease_off", cube, {"levels": 2, "use_creases": False}, creases({0: 1.0, 1: 1.0}))
run("cube_l2_vcrease", cube, {"levels": 2}, creases({}, {0: 1.0, 3: 0.4}))
for mode in ("NONE", "PRESERVE_CORNERS", "PRESERVE_CORNERS_AND_JUNCTIONS",
             "PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE", "PRESERVE_BOUNDARIES", "SMOOTH_ALL"):
    run("cube_l2_uv_" + mode.lower(), cube, {"levels": 2, "uv_smooth": mode})


def grid():
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, size=2)


run("grid_l2", grid, {"levels": 2})
run("grid_l2_corners", grid, {"levels": 2, "boundary_smooth": "PRESERVE_CORNERS"})
run("grid_l2_uv_smooth_all", grid, {"levels": 2, "uv_smooth": "SMOOTH_ALL"})
run("grid_l2_uv_corners", grid, {"levels": 2, "uv_smooth": "PRESERVE_CORNERS"})


def plane():
    bpy.ops.mesh.primitive_plane_add()


run("plane_l1", plane, {"levels": 1})
run("plane_l3_crease", plane, {"levels": 3}, creases({0: 1.0, 1: 1.0, 2: 1.0, 3: 1.0}))


def cylinder():
    bpy.ops.mesh.primitive_cylinder_add(vertices=6)


for lv in (1, 2, 3):
    run("cylinder_l%d" % lv, cylinder, {"levels": lv})
run("cylinder_l2_uv_corners", cylinder, {"levels": 2, "uv_smooth": "PRESERVE_CORNERS"})


def cone():
    bpy.ops.mesh.primitive_cone_add(vertices=5)


run("cone_l2", cone, {"levels": 2})
run("cone_l1", cone, {"levels": 1})


def monkey():
    bpy.ops.mesh.primitive_monkey_add()


run("monkey_l1", monkey, {"levels": 1})


def tri():
    me = bpy.data.meshes.new("Tri")
    me.from_pydata([(0, 0, 0), (1, 0, 0), (0, 1, 0.5), (1, 1, 0.2)], [], [(0, 1, 2), (1, 3, 2)])
    ob = bpy.data.objects.new("Tri", me)
    bpy.context.scene.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob


run("tris_l2", tri, {"levels": 2})


def loose():
    me = bpy.data.meshes.new("Loose")
    me.from_pydata(
        [(0, 0, 0), (1, 0, 0), (1, 1, 0.3), (0, 1, 0), (2, 0, 0), (3, 0.5, 0), (4, 0, 1), (5, 5, 5), (2, 2, 0)],
        [(1, 4), (4, 5), (5, 6), (2, 8)],
        [(0, 1, 2, 3)])
    ob = bpy.data.objects.new("Loose", me)
    bpy.context.scene.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob


run("loose_l2", loose, {"levels": 2})
run("loose_l2_simple", loose, {"levels": 2, "subdivision_type": "SIMPLE"})


def nonmanifold():
    me = bpy.data.meshes.new("NM")
    me.from_pydata(
        [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (1, 0, 1), (0, 0, 1), (1, 0, -1), (0, 0, -1),
         (0.5, -1, 0.2)],
        [],
        [(0, 1, 2, 3), (1, 0, 5, 4), (0, 1, 6, 7), (0, 8, 1)])
    ob = bpy.data.objects.new("NM", me)
    bpy.context.scene.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob


run("nonmanifold_l2", nonmanifold, {"levels": 2})


def torus_ngon():
    bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type="NGON")


run("ngon_l2", torus_ngon, {"levels": 2})
run("ngon_l3", torus_ngon, {"levels": 3})


def sphere():
    bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=5)


run("sphere_l2", sphere, {"levels": 2})


# SIMPLE past level 1 (Gregory end caps around the level-1 face points of non-quads) and
# levels above quality (Gregory end caps at the isolation level). Suzanne at SIMPLE level 2 is a
# golden case (`tests/golden/cases/modifiers/subsurf/simple_end_caps.py`).
for lv in (2, 3):
    run("cone_l%d_simple" % lv, cone, {"levels": lv, "subdivision_type": "SIMPLE"})
    run("cylinder_l%d_simple" % lv, cylinder, {"levels": lv, "subdivision_type": "SIMPLE"})
    run("sphere_l%d_simple" % lv, sphere, {"levels": lv, "subdivision_type": "SIMPLE"})
run("tris_l3_simple", tri, {"levels": 3, "subdivision_type": "SIMPLE"})
run("cylinder_l4", cylinder, {"levels": 4})
# Level above quality on a mesh with smooth boundary corners (one face) and an extraordinary
# face point: the faces around the corners keep frap's own evaluation (D-121).
run("ngon_l4", torus_ngon, {"levels": 4})
