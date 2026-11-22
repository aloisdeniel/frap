import bpy, json, sys, math

out_path = sys.argv[sys.argv.index("--") + 1]


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def mesh_json(me):
    d = {
        "positions": [list(v.co) for v in me.vertices],
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
        "material_index": [p.material_index for p in me.polygons],
        "materials": [m.name if m else None for m in me.materials],
        "uv": {},
        "attrs": {},
    }
    for uv in me.uv_layers:
        d["uv"][uv.name] = [list(l.uv) for l in uv.data]
    for a in me.attributes:
        if a.name.startswith(".") or a.name in ("position",) or a.data_type not in ("FLOAT", "INT", "BOOLEAN"):
            continue
        d["attrs"][a.name] = [a.domain, a.data_type, [x.value for x in a.data]]
    return d


def ob_json(ob):
    return {
        "name": ob.name,
        "matrix_world": [list(r) for r in ob.matrix_world],
        "mesh": mesh_json(ob.data),
        "slots": [s.material.name if s.material else None for s in ob.material_slots],
    }


def mat(name):
    return bpy.data.materials.get(name) or bpy.data.materials.new(name)


def evaluated(ob):
    dg = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(dg)
    me = ev.to_mesh()
    d = mesh_json(me)
    ev.to_mesh_clear()
    return d


cases = {}


def run(name, setup, solver="MANIFOLD", operation="DIFFERENCE", material_mode="INDEX", use_self=False, hole_tolerant=False, collection=False):
    reset()
    base, ops = setup()
    m = base.modifiers.new("Boolean", "BOOLEAN")
    m.solver = solver
    m.operation = operation
    m.material_mode = material_mode
    m.use_self = use_self
    m.use_hole_tolerant = hole_tolerant
    if collection:
        col = bpy.data.collections.new("Cutters")
        bpy.context.scene.collection.children.link(col)
        for o in ops:
            for c in list(o.users_collection):
                c.objects.unlink(o)
            col.objects.link(o)
        m.operand_type = "COLLECTION"
        m.collection = col
    else:
        m.object = ops[0]
    for o in ops:
        o.hide_set(True)
    bpy.context.view_layer.update()
    res = evaluated(base)
    err = None
    cases[name] = {
        "solver": solver,
        "operation": operation,
        "material_mode": material_mode,
        "base": ob_json(base),
        "operands": [ob_json(o) for o in ops],
        "result": res,
    }


def cube_cube():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0.5, 0.5, 0.5))
    b = bpy.context.object
    return a, [b]


def cube_cyl():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cylinder_add(radius=0.5, depth=3, vertices=12)
    b = bpy.context.object
    return a, [b]


def cube_sphere():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_uv_sphere_add(radius=1.2, segments=16, ring_count=8, location=(0.9, 0.8, 0.7))
    b = bpy.context.object
    return a, [b]


def cube_cube_rot():
    bpy.ops.mesh.primitive_cube_add(size=2, location=(0.3, 0, 0), rotation=(0, 0, 0.4), scale=(1, 1.5, 1))
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(1.0, 0.8, 0.7), rotation=(0.3, 0.2, 0.1))
    b = bpy.context.object
    return a, [b]


def mats():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    a.data.materials.append(mat("Red"))
    a.data.materials.append(mat("Green"))
    for i, p in enumerate(a.data.polygons):
        p.material_index = i % 2
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0.5, 0.5, 0.5))
    b = bpy.context.object
    b.data.materials.append(mat("Blue"))
    b.data.materials.append(mat("Red"))
    for i, p in enumerate(b.data.polygons):
        p.material_index = (i // 2) % 2
    return a, [b]


def two_cutters():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(1, 1, 1))
    b = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(-1, -1, -1))
    c = bpy.context.object
    return a, [b, c]


def plane_cut():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_plane_add(size=4, location=(0, 0, 0.25), rotation=(0.2, 0, 0))
    b = bpy.context.object
    return a, [b]


def nonmanifold():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, size=3)
    b = bpy.context.object
    return a, [b]


def disjoint():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(4, 0, 0))
    b = bpy.context.object
    return a, [b]


def torus_cube():
    bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(1, 0, 0))
    b = bpy.context.object
    return a, [b]


def ico_cyl():
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1.0)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.4, depth=3, location=(0.1, 0.2, 0.0), rotation=(0.5, 0.3, 0.0))
    b = bpy.context.object
    return a, [b]


def cyl_cyl():
    bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.5, depth=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.4, depth=2, rotation=(1.5707963, 0, 0), location=(0.05, 0.0, 0.1))
    b = bpy.context.object
    return a, [b]


def mirrored():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0.6, 0.7, 0.8), scale=(-1.2, 1, 1))
    b = bpy.context.object
    return a, [b]


for solver in ("MANIFOLD", "EXACT"):
    s = solver.lower()
    run(f"{s}_ico_cyl", ico_cyl, solver)
    run(f"{s}_cyl_cyl_union", cyl_cyl, solver, "UNION")
    run(f"{s}_cyl_cyl_intersect", cyl_cyl, solver, "INTERSECT")
    run(f"{s}_mirrored", mirrored, solver)
    for op in ("DIFFERENCE", "UNION", "INTERSECT"):
        run(f"{s}_cube_cube_{op.lower()}", cube_cube, solver, op)
    run(f"{s}_cube_cyl", cube_cyl, solver)
    run(f"{s}_cube_sphere", cube_sphere, solver)
    run(f"{s}_cube_sphere_union", cube_sphere, solver, "UNION")
    run(f"{s}_cube_cube_rot", cube_cube_rot, solver)
    run(f"{s}_mats_index", mats, solver, "UNION", "INDEX")
    run(f"{s}_mats_transfer", mats, solver, "UNION", "TRANSFER")
    run(f"{s}_collection", two_cutters, solver, "DIFFERENCE", collection=True)
    run(f"{s}_plane_cut", plane_cut, solver)
    run(f"{s}_nonmanifold", nonmanifold, solver)
    run(f"{s}_disjoint_union", disjoint, solver, "UNION")
    run(f"{s}_disjoint_intersect", disjoint, solver, "INTERSECT")
    run(f"{s}_torus_cube", torus_cube, solver)


# FLOAT (WP 8.6, BM_mesh_intersect): the same setups, T2 (tests.rs `float_matches_blender_t2`).
# A COLLECTION operand with INTERSECT is Blender's refusal (checked separately); results that
# change between Blender runs (pointer-hash order inside BM_mesh_intersect) are listed in
# tests.rs (`FLOAT_NONDETERMINISTIC`).
run("float_ico_cyl", ico_cyl, "FLOAT")
run("float_cyl_cyl_union", cyl_cyl, "FLOAT", "UNION")
run("float_cyl_cyl_intersect", cyl_cyl, "FLOAT", "INTERSECT")
run("float_mirrored", mirrored, "FLOAT")
for op in ("DIFFERENCE", "UNION", "INTERSECT"):
    run(f"float_cube_cube_{op.lower()}", cube_cube, "FLOAT", op)
    run(f"float_cube_cube_rot_{op.lower()}", cube_cube_rot, "FLOAT", op)
run("float_cube_cyl", cube_cyl, "FLOAT")
run("float_cube_sphere", cube_sphere, "FLOAT")
run("float_cube_sphere_union", cube_sphere, "FLOAT", "UNION")
run("float_mats_index", mats, "FLOAT", "UNION", "INDEX")
run("float_collection", two_cutters, "FLOAT", "DIFFERENCE", collection=True)
run("float_collection_union", two_cutters, "FLOAT", "UNION", collection=True)
run("float_plane_cut", plane_cut, "FLOAT")
run("float_nonmanifold", nonmanifold, "FLOAT")
run("float_disjoint_union", disjoint, "FLOAT", "UNION")
run("float_disjoint_intersect", disjoint, "FLOAT", "INTERSECT")
run("float_torus_cube", torus_cube, "FLOAT")


# Wider MANIFOLD coverage (verifier probes): large offsets, coincident operands, n-gons,
# subdivided faces, operand slots, nested collections, mixed scale, thin cutters, drills,
# duplicate and empty slots. The ones frap does not match at T2 are the known gaps listed in
# tests.rs (KNOWN_GAPS), checked at T3.
def big():
    bpy.ops.mesh.primitive_cube_add(size=1000, location=(1e4, 2e4, -3e4))
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=600, location=(1e4 + 400, 2e4 + 300, -3e4 + 200), rotation=(0.1, 0.2, 0.3))
    return a, [bpy.context.object]


def same():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=2)
    return a, [bpy.context.object]


def ngon():
    bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=1, depth=1)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=0.6, location=(0.2, 0.1, 0.5), rotation=(0.2, 0.3, 0.4))
    return a, [bpy.context.object]


def subdiv():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.subdivide(number_cuts=2)
    bpy.ops.object.mode_set(mode='OBJECT')
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.9, segments=10, ring_count=6, location=(0.83, -0.41, 0.57))
    return a, [bpy.context.object]


def mat_noslot():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.4, depth=3, location=(0.3, 0.2, 0), rotation=(0.3, 0.1, 0))
    b = bpy.context.object
    b.data.materials.append(mat("Blue"))
    b.data.materials.append(mat("Green"))
    for i, p in enumerate(b.data.polygons):
        p.material_index = i % 2
    return a, [b]


def nested():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(1, 1, 1))
    b = bpy.context.object
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.7, location=(-1, -0.9, -0.8))
    c = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=0.5, location=(1, -1, 0.3), rotation=(0.5, 0.5, 0.5))
    d = bpy.context.object
    return a, [b, c, d]


def mixedscale():
    bpy.ops.mesh.primitive_cube_add(size=2, scale=(2, 0.5, 1), rotation=(0.3, 0, 0.2))
    a = bpy.context.object
    bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.4, depth=3, location=(0.6, 0.2, 0.1), scale=(1, 2, 0.7), rotation=(0, 1.2, 0.3))
    return a, [bpy.context.object]


def thin():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0.3, 0.2, 0.9999), scale=(1, 1, 0.001))
    return a, [bpy.context.object]


def cyl(**kw):
    def f():
        bpy.ops.mesh.primitive_cube_add(size=2)
        a = bpy.context.object
        bpy.ops.mesh.primitive_cylinder_add(**kw)
        return a, [bpy.context.object]
    return f


def three():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    r = []
    for rot in [(0, 0, 0), (1.5707963, 0, 0), (0, 1.5707963, 0)]:
        bpy.ops.mesh.primitive_cylinder_add(radius=0.5, depth=3, rotation=rot)
        r.append(bpy.context.object)
    return a, r


def sph():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    bpy.ops.mesh.primitive_uv_sphere_add(location=(1, 0, 0))
    return a, [bpy.context.object]


def notch():
    bpy.ops.mesh.primitive_cylinder_add()
    a = bpy.context.object
    bpy.ops.mesh.primitive_cube_add(size=0.5, location=(1, 0, 1))
    return a, [bpy.context.object]


def dupslots():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    for n in ("Green", "Red", "Red"):
        a.data.materials.append(mat(n))
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0.9, 0.8, 0.7))
    b = bpy.context.object
    b.data.materials.append(mat("Red"))
    return a, [b]


def emptyslots():
    bpy.ops.mesh.primitive_cube_add(size=2)
    a = bpy.context.object
    a.data.materials.append(mat("Green"))
    a.data.materials.append(None)
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0.9, 0.8, 0.7))
    b = bpy.context.object
    b.data.materials.append(mat("Blue"))
    b.data.materials.append(None)
    for i, p in enumerate(b.data.polygons):
        p.material_index = i % 2
    return a, [b]


run("manifold_big_offset", big)
run("manifold_same_union", same, operation="UNION")
run("manifold_same_difference", same)
run("manifold_same_intersect", same, operation="INTERSECT")
run("manifold_ngon", ngon)
run("manifold_ngon_intersect", ngon, operation="INTERSECT")
run("manifold_subdiv", subdiv)
run("manifold_noslot_index", mat_noslot, operation="UNION")
run("manifold_noslot_transfer", mat_noslot, operation="UNION", material_mode="TRANSFER")
run("manifold_nested", nested, collection=True)
run("manifold_nested_union", nested, operation="UNION", collection=True)
run("manifold_mixedscale", mixedscale)
run("manifold_mixedscale_union", mixedscale, operation="UNION")
run("manifold_thin", thin)
run("manifold_cyl_coplanar", cyl(radius=0.3, depth=2))
run("manifold_drill32", cyl(radius=0.3, depth=3, vertices=32))
run("manifold_drill_x", cyl(radius=0.4, depth=3, vertices=16, rotation=(1.5707963, 0, 0), location=(0.2, 0, 0.1)))
run("manifold_drill_off", cyl(radius=0.25, depth=3, vertices=12, location=(0.3, -0.2, 0)))
run("manifold_three_cylinders", three, collection=True)
run("manifold_sphere_union", sph, operation="UNION")
run("manifold_notch", notch)
run("manifold_dupslots_index", dupslots, operation="UNION")
run("manifold_dupslots_transfer", dupslots, operation="UNION", material_mode="TRANSFER")
run("manifold_emptyslots_index", emptyslots, operation="UNION")
run("manifold_emptyslots_transfer", emptyslots, operation="UNION", material_mode="TRANSFER")

with open(out_path, "w") as f:
    json.dump(cases, f)
print("wrote", len(cases))
