# SPDX-License-Identifier: GPL-3.0-or-later
# WP 8.11: bit-exact face-varying reference dumps (Blender 5.2.4), read by
# `tests/fvar_dumps.rs`. Every case builds a mesh, adds a SUBSURF modifier and writes the base
# mesh topology, its UV maps and creases (exact float bits), the settings, and the subdivided
# UV maps Blender evaluated (`eval_face_varying` through OpenSubdiv): every corner's bits when
# the mesh is small, else an FNV-1a hash of them, plus the number of distinct
# (vertex, u, v) corner tuples (what the glTF exporter splits on). Positions are not written:
# face-varying evaluation does not depend on them.
# Usage: Blender -b --factory-startup --python dump_fvar.py -- OUTDIR [filter]
import bpy, json, sys, os, struct

argv = sys.argv[sys.argv.index("--") + 1:]
OUT = argv[0]
FILT = argv[1] if len(argv) > 1 else None
os.makedirs(OUT, exist_ok=True)
FULL_LIMIT = 6000


def bits(x):
    return struct.unpack("<I", struct.pack("<f", x))[0]


def uv_hex(layer):
    return "".join("%08x%08x" % (bits(l.uv[0]), bits(l.uv[1])) for l in layer.data)


def fnv(words):
    h = 0xcbf29ce484222325
    for w in words:
        for b in struct.pack("<I", w):
            h ^= b
            h = (h * 0x100000001b3) & 0xffffffffffffffff
    return "%016x" % h


def clear():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob)
    for me in list(bpy.data.meshes):
        bpy.data.meshes.remove(me)


def attr(me, name):
    a = me.attributes.get(name)
    if a is None:
        return None
    return ["%08x" % bits(x.value) for x in a.data]


def run(name, build, settings, edit=None):
    if FILT and FILT not in name:
        return
    clear()
    build()
    ob = bpy.context.view_layer.objects.active
    if edit:
        edit(ob)
    me = ob.data
    inp = {
        "num_vertices": len(me.vertices),
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
        "corner_edges": [l.edge_index for l in me.loops],
        "uvs": [uv_hex(l) for l in me.uv_layers],
        "crease_edge": attr(me, "crease_edge"),
        "crease_vert": attr(me, "crease_vert"),
    }
    md = ob.modifiers.new("Subdivision", "SUBSURF")
    for k, v in settings.items():
        setattr(md, k, v)
    dg = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(dg)
    em = ev.to_mesh()
    corner_verts = [l.vertex_index for l in em.loops]
    layers = []
    for layer in em.uv_layers:
        words = []
        for l in layer.data:
            words.append(bits(l.uv[0]))
            words.append(bits(l.uv[1]))
        distinct = len(set(zip(corner_verts, words[0::2], words[1::2])))
        layers.append({
            "hash": fnv(words),
            "distinct": distinct,
            "bits": uv_hex(layer) if len(corner_verts) <= FULL_LIMIT else None,
        })
    out = {"corners": len(corner_verts), "uvs": layers}
    ev.to_mesh_clear()
    s = {k: getattr(md, k) for k in ("levels", "quality", "use_limit_surface", "subdivision_type",
                                    "boundary_smooth", "uv_smooth", "use_creases")}
    with open(os.path.join(OUT, name + ".json"), "w") as f:
        json.dump({"name": name, "settings": s, "input": inp, "output": out}, f,
                  separators=(",", ":"))
    print("wrote", name, out["corners"], [l["distinct"] for l in layers])


def prim(kind, **kw):
    def f():
        getattr(bpy.ops.mesh, "primitive_%s_add" % kind)(**kw)
    return f


def smart_mirrored():
    bpy.ops.mesh.primitive_monkey_add()
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project()
    bpy.ops.object.mode_set(mode="OBJECT")
    ob = bpy.context.object
    ob.modifiers.new("Mirror", "MIRROR")
    bpy.ops.object.modifier_apply(modifier="Mirror")


def two_uv_maps(ob):
    me = ob.data
    me.uv_layers.new(name="Second")
    me.uv_layers.active_index = 1
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.cube_project()
    bpy.ops.object.mode_set(mode="OBJECT")


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


MODES = ("NONE", "PRESERVE_CORNERS", "PRESERVE_CORNERS_AND_JUNCTIONS",
         "PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE", "PRESERVE_BOUNDARIES", "SMOOTH_ALL")

monkey = prim("monkey")
cube = prim("cube")
sphere = prim("uv_sphere")
cylinder = prim("cylinder")
cylinder6 = prim("cylinder", vertices=6)
cone = prim("cone", vertices=5)
grid = prim("grid", x_subdivisions=4, y_subdivisions=3)
ngon7 = prim("circle", vertices=7, fill_type="NGON")

for lv in (1, 2):
    run("monkey_l%d" % lv, monkey, {"levels": lv})
for lv in (1, 2, 3):
    run("cube_l%d" % lv, cube, {"levels": lv})
for lv in (1, 2):
    run("sphere_l%d" % lv, sphere, {"levels": lv})
    run("cylinder_l%d" % lv, cylinder, {"levels": lv})
# SIMPLE past level 1 with n-gons (Gregory end caps around the level-1 face points).
for lv in (2, 3):
    run("cylinder_simple_l%d" % lv, cylinder6, {"levels": lv, "subdivision_type": "SIMPLE"})
    run("cone_simple_l%d" % lv, cone, {"levels": lv, "subdivision_type": "SIMPLE"})
    run("ngon7_simple_l%d" % lv, ngon7, {"levels": lv, "subdivision_type": "SIMPLE"})
run("monkey_simple_l2", monkey, {"levels": 2, "subdivision_type": "SIMPLE"})
run("cylinder32_simple_l2", cylinder, {"levels": 2, "subdivision_type": "SIMPLE"})
# Every uv_smooth.
for mode in MODES:
    m = mode.lower()
    run("cube_l2_uv_" + m, cube, {"levels": 2, "uv_smooth": mode})
    run("monkey_l1_uv_" + m, monkey, {"levels": 1, "uv_smooth": mode})
    run("grid_l2_uv_" + m, grid, {"levels": 2, "uv_smooth": mode})
    run("cylinder6_l2_uv_" + m, cylinder6, {"levels": 2, "uv_smooth": mode})
# Levels above quality (Gregory end caps at the isolation level).
run("cube_l3_q1", cube, {"levels": 3, "quality": 1})
run("cube_l4", cube, {"levels": 4})
run("cylinder6_l4", cylinder6, {"levels": 4})
run("monkey_l2_q1", monkey, {"levels": 2, "quality": 1})
run("sphere_l3_q2", sphere, {"levels": 3, "quality": 2})
run("ngon7_l4", ngon7, {"levels": 4})
run("grid_l3_q1", grid, {"levels": 3, "quality": 1})
# Without the limit surface (uniform refinement, bilinear patches).
run("cube_l2_nolimit", cube, {"levels": 2, "use_limit_surface": False})
run("monkey_l2_nolimit", monkey, {"levels": 2, "use_limit_surface": False})
run("cylinder6_simple_l2_nolimit", cylinder6, {"levels": 2, "subdivision_type": "SIMPLE", "use_limit_surface": False})
# Boundaries, creases, two UV maps, the smart-projected mirrored Suzanne.
run("grid_l2_corners", grid, {"levels": 2, "boundary_smooth": "PRESERVE_CORNERS"})
run("cube_l2_crease", cube, {"levels": 2}, creases({0: 1.0, 1: 0.5, 2: 0.8, 5: 0.3, 7: 0.25}))
run("cube_l3_vcrease", cube, {"levels": 3}, creases({}, {0: 1.0, 3: 0.4}))
run("monkey_l1_crease", monkey, {"levels": 1}, creases({i: 0.1 * (i % 11) for i in range(0, 1000, 7)}))
run("grid_l2_crease_uv_corners", grid, {"levels": 2, "uv_smooth": "PRESERVE_CORNERS"}, creases({0: 0.6, 3: 1.0, 9: 0.4}))
run("cube_l2_two_uvs", cube, {"levels": 2}, two_uv_maps)
run("monkey_mirror_smart_l2", smart_mirrored, {"levels": 2})
