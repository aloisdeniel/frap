# SPDX-License-Identifier: GPL-3.0-or-later
"""Build the Blender-exported glTF fixtures and dump Blender's `import_scene.gltf` result
(ARCHITECTURE §17.2, §22.3 WP 1.17).

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup -t 8 --python tools/blender/make_gltf_fixtures.py -- \
        --out tests/data/gltf-blender [--filter <substring>] [--no-export]

Two steps:

1. Fixtures (`<out>/fixtures/<name>.glb`): small scenes built with bpy and exported with
   `export_scene.gltf` at its defaults (plus `use_mesh_edges`/`use_mesh_vertices` for the loose
   geometry fixture). `--no-export` keeps the committed GLBs, so re-dumping on another Blender
   build does not churn them.
   Fixtures are written to `<out>/fixtures`; cases read them from there, falling back to the
   committed copies.
2. Dumps (`<out>/dumps/<case>.json`): for every case (a fixture or a vendored Khronos sample
   from `tests/data/gltf-samples`, plus import settings), start from
   `read_factory_settings(use_empty=True)`, run `import_scene.gltf(filepath=…, **settings)` and
   write the resulting state: scenes (collection tree, view layers, bases, selection, active
   object), `bpy.data` collection order, objects, meshes (positions, edges, faces, corner
   normals, UV maps, every non-internal attribute, material slots), materials (non-node
   properties and the node graph in the GoldenState shape of `dump_state.py`), images,
   cameras and lights.

`crates/frap-io-gltf/tests/import_vs_blender.rs` reads the dumps, runs frap's importer in-process
on the same source with the same settings, and compares field by field (`frap-io-gltf` cannot
dev-depend on `frap-golden`, rule 2.2.11).

Always pass `-t 8` (see `dump_primitives.py`: `mesh_calc_edges` partitions large meshes by the
thread count). Floats are the shortest decimal that round-trips the stored f32.
"""

import json
import math
import os
import struct
import sys

import bpy

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SAMPLES = "tests/data/gltf-samples"

# --------------------------------------------------------------------------- fixture scenes


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def link(ob, coll=None):
    (coll or bpy.context.scene.collection).objects.link(ob)
    return ob


def principled(name, color=(0.8, 0.8, 0.8, 1.0), metallic=0.0, roughness=0.5):
    mat = bpy.data.materials.new(name)
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    return mat


def fx_cube():
    """The factory cube: flat normals, one UV map, the factory material."""
    bpy.ops.wm.read_factory_settings(use_empty=False)
    for name in ("Camera", "Light"):
        bpy.data.objects.remove(bpy.data.objects[name])


def fx_sphere_smooth():
    reset()
    bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.0)
    bpy.ops.object.shade_smooth()
    bpy.context.object.data.materials.append(principled("Glossy", (0.1, 0.4, 0.9, 1.0), 0.0, 0.2))


def fx_hierarchy():
    reset()
    root = link(bpy.data.objects.new("Root", None))
    root.location = (1.0, 2.0, 3.0)
    root.rotation_euler = (0.3, 0.2, 0.1)
    root.scale = (1.0, 2.0, 1.0)
    bpy.ops.mesh.primitive_cube_add(size=1.0)
    cube = bpy.context.object
    cube.name = "Cube.001"
    cube.parent = root
    cube.location = (0.5, -1.0, 0.25)
    cube.rotation_euler = (0.0, 0.0, 0.7)
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.5)
    ico = bpy.context.object
    ico.name = "Ico"
    ico.parent = cube
    ico.location = (0.0, 0.0, 1.5)
    ico.scale = (0.5, 1.0, 1.5)
    bpy.ops.mesh.primitive_cone_add(vertices=6)
    cone = bpy.context.object
    cone.rotation_mode = "QUATERNION"
    cone.rotation_quaternion = (0.9, 0.1, 0.3, -0.2)
    cone.location = (-2.0, 0.0, 0.0)
    leaf = link(bpy.data.objects.new("Leaf", None))
    leaf.parent = cone
    leaf.location = (0.0, 0.0, 2.0)


def fx_multi_material():
    reset()
    bpy.ops.mesh.primitive_cube_add()
    me = bpy.context.object.data
    me.materials.append(principled("Red", (0.8, 0.05, 0.05, 1.0), 0.5, 0.2))
    me.materials.append(principled("Blue", (0.05, 0.05, 0.8, 1.0), 0.0, 0.9))
    me.materials.append(principled("Unused", (0.0, 1.0, 0.0, 1.0)))
    for i, p in enumerate(me.polygons):
        p.material_index = 0 if i < 2 else 1


def fx_mixed_shading():
    """Smooth sides, flat caps: import_shading NORMALS must find the flat faces."""
    reset()
    bpy.ops.mesh.primitive_cylinder_add(vertices=8, depth=2.0)
    ob = bpy.context.object
    bpy.ops.object.shade_smooth()
    for p in ob.data.polygons:
        if len(p.vertices) > 4:
            p.use_smooth = False


def fx_textured():
    reset()
    bpy.ops.mesh.primitive_plane_add(size=2.0)
    ob = bpy.context.object
    img = bpy.data.images.new("Checker", 8, 8)
    img.generated_type = "COLOR_GRID"
    mat = principled("Textured")
    tree = mat.node_tree
    tex = tree.nodes.new("ShaderNodeTexImage")
    tex.image = img
    tree.links.new(tex.outputs["Color"], tree.nodes["Principled BSDF"].inputs["Base Color"])
    ob.data.materials.append(mat)


def fx_shared_mesh():
    reset()
    bpy.ops.mesh.primitive_cube_add(size=1.0)
    a = bpy.context.object
    b = link(bpy.data.objects.new("Twin", a.data))
    b.location = (3.0, 0.0, 0.0)


def fx_loose():
    """Faces plus loose edges and loose points (LINES and POINTS primitives)."""
    reset()
    me = bpy.data.meshes.new("Loose")
    verts = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 0, 0), (3, 0, 0), (3, 1, 0), (5, 5, 5), (6, 5, 5)]
    edges = [(4, 5), (5, 6)]
    faces = [(0, 1, 2, 3)]
    me.from_pydata(verts, edges, faces)
    link(bpy.data.objects.new("Loose", me))


FIXTURES = [
    ("cube", fx_cube, {}),
    ("sphere_smooth", fx_sphere_smooth, {}),
    ("hierarchy", fx_hierarchy, {}),
    ("multi_material", fx_multi_material, {}),
    ("mixed_shading", fx_mixed_shading, {}),
    ("textured", fx_textured, {}),
    ("shared_mesh", fx_shared_mesh, {}),
    ("loose", fx_loose, {"use_mesh_edges": True, "use_mesh_vertices": True}),
]

# --------------------------------------------------------------------------- import cases


FIXTURES_DIR = "tests/data/gltf-blender/fixtures"


def fixture(name):
    return "%s/%s.glb" % (FIXTURES_DIR, name)


def sample(path):
    return "%s/%s" % (SAMPLES, path)


# (case name, source path relative to the repository, import settings)
CASES = [
    ("cube", fixture("cube"), {}),
    ("cube__merge", fixture("cube"), {"merge_vertices": True}),
    ("sphere_smooth", fixture("sphere_smooth"), {}),
    ("sphere_smooth__merge", fixture("sphere_smooth"), {"merge_vertices": True}),
    ("sphere_smooth__flat", fixture("sphere_smooth"), {"import_shading": "FLAT"}),
    ("hierarchy", fixture("hierarchy"), {}),
    ("hierarchy__noselect", fixture("hierarchy"), {"import_select_created_objects": False}),
    ("multi_material", fixture("multi_material"), {}),
    ("multi_material__nomerge_slots", fixture("multi_material"), {"import_merge_material_slots": False}),
    ("mixed_shading", fixture("mixed_shading"), {}),
    ("mixed_shading__merge", fixture("mixed_shading"), {"merge_vertices": True}),
    ("mixed_shading__smooth", fixture("mixed_shading"), {"import_shading": "SMOOTH"}),
    ("textured", fixture("textured"), {}),
    ("shared_mesh", fixture("shared_mesh"), {}),
    ("loose", fixture("loose"), {}),
    ("loose__merge", fixture("loose"), {"merge_vertices": True}),
    # Khronos subset (static content only: skins, morph targets and animations arrive in M6,
    # instancing in M7).
    ("khr_box", sample("Box/glTF-Binary/Box.glb"), {}),
    ("khr_box_interleaved", sample("BoxInterleaved/glTF-Binary/BoxInterleaved.glb"), {}),
    ("khr_box_vertex_colors", sample("BoxVertexColors/glTF-Binary/BoxVertexColors.glb"), {}),
    ("khr_mesh_primitive_modes", sample("MeshPrimitiveModes/glTF-Embedded/MeshPrimitiveModes.gltf"), {}),
    ("khr_mesh_primitive_modes__merge", sample("MeshPrimitiveModes/glTF-Embedded/MeshPrimitiveModes.gltf"),
     {"merge_vertices": True}),
    ("khr_multiple_scenes", sample("MultipleScenes/glTF-Embedded/MultipleScenes.gltf"), {}),
    ("khr_simple_material", sample("SimpleMaterial/glTF-Embedded/SimpleMaterial.gltf"), {}),
    ("khr_simple_meshes", sample("SimpleMeshes/glTF-Embedded/SimpleMeshes.gltf"), {}),
    ("khr_simple_sparse_accessor", sample("SimpleSparseAccessor/glTF-Embedded/SimpleSparseAccessor.gltf"), {}),
    ("khr_triangle", sample("Triangle/glTF/Triangle.gltf"), {}),
    ("khr_triangle_without_indices", sample("TriangleWithoutIndices/glTF-Embedded/TriangleWithoutIndices.gltf"), {}),
    ("khr_texture_transform", sample("TextureTransformTest/glTF/TextureTransformTest.gltf"), {}),
    ("khr_unlit", sample("UnlitTest/glTF-Binary/UnlitTest.glb"), {}),
    ("khr_emissive_strength", sample("EmissiveStrengthTest/glTF-Binary/EmissiveStrengthTest.glb"), {}),
    ("khr_cameras", sample("Cameras/glTF-Embedded/Cameras.gltf"), {}),
    ("khr_point_light", sample("PointLightIntensityTest/glTF-Binary/PointLightIntensityTest.glb"), {}),
]

# --------------------------------------------------------------------------- dump


def f32(x):
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


def name_of(idblock):
    return None if idblock is None else idblock.name


def attribute_data(a):
    field = {"FLOAT2": "vector", "FLOAT_VECTOR": "vector", "FLOAT_COLOR": "color", "BYTE_COLOR": "color"}.get(
        a.data_type, "value")
    out = []
    for d in a.data:
        v = getattr(d, field)
        if isinstance(v, bool) or isinstance(v, int):
            out.append(v)
        elif isinstance(v, float):
            out.append(f32(v))
        elif isinstance(v, str):
            out.append(v)
        else:
            out.append([f32(x) if isinstance(x, float) else x for x in v])
    return out


def dump_layer_collection(lc):
    return {
        "collection": lc.collection.name,
        "exclude": lc.exclude,
        "children": [dump_layer_collection(c) for c in lc.children],
    }


def dump_scene(sc):
    return {
        "name": sc.name,
        "collection": {
            "objects": [o.name for o in sc.collection.objects],
            "children": [c.name for c in sc.collection.children],
        },
        "view_layers": [
            {
                "name": vl.name,
                "objects": [o.name for o in vl.objects],
                "selected": [o.name for o in vl.objects if o.select_get(view_layer=vl)],
                "active": name_of(vl.objects.active),
                "layer_collection": dump_layer_collection(vl.layer_collection),
            }
            for vl in sc.view_layers
        ],
    }


def dump_object(ob):
    return {
        "name": ob.name,
        "type": ob.type,
        "data": name_of(ob.data),
        "parent": name_of(ob.parent),
        "parent_type": ob.parent_type,
        "location": fvec(ob.location),
        "rotation_mode": ob.rotation_mode,
        "rotation_quaternion": fvec(ob.rotation_quaternion),
        "scale": fvec(ob.scale),
        "empty_display_size": f32(ob.empty_display_size),
        "material_slots": [[s.link, name_of(s.material)] for s in ob.material_slots],
        "users_collection": [c.name for c in ob.users_collection],
    }


def dump_mesh(me):
    uv_names = [l.name for l in me.uv_layers]
    attributes = []
    for a in me.attributes:
        if a.name.startswith(".") or a.name == "position" or a.name in uv_names:
            continue
        attributes.append({"name": a.name, "domain": a.domain, "data_type": a.data_type, "data": attribute_data(a)})
    return {
        "name": me.name,
        "positions": [fvec(v.co) for v in me.vertices],
        "edges": [list(e.vertices) for e in me.edges],
        "faces": [list(p.vertices) for p in me.polygons],
        "corner_normals": [fvec(n.vector) for n in me.corner_normals],
        "normals_domain": me.normals_domain,
        "uv_layers": [{"name": l.name, "data": [fvec(d.uv) for d in l.data]} for l in me.uv_layers],
        "attribute_names": [a.name for a in me.attributes],
        "attributes": attributes,
        "materials": [name_of(m) for m in me.materials],
    }


def dump_material(mat):
    nodes = {}
    tree = mat.node_tree
    for n in tree.nodes:
        inputs = {}
        for s in n.inputs:
            if not hasattr(s, "default_value"):
                continue
            v = s.default_value
            if isinstance(v, (bool, int, str)):
                inputs[s.identifier] = v
            elif isinstance(v, float):
                inputs[s.identifier] = f32(v)
            elif v is None:
                inputs[s.identifier] = None
            elif hasattr(v, "name") and hasattr(v, "users"):
                inputs[s.identifier] = v.name
            else:
                inputs[s.identifier] = fvec(v)
        nodes[n.name] = {"bl_idname": n.bl_idname, "mute": n.mute, "inputs": inputs}
    links = sorted(
        [l.from_node.name, l.from_socket.identifier, l.to_node.name, l.to_socket.identifier] for l in tree.links)
    return {
        "name": mat.name,
        "use_backface_culling": mat.use_backface_culling,
        "surface_render_method": mat.surface_render_method,
        "diffuse_color": fvec(mat.diffuse_color),
        "metallic": f32(mat.metallic),
        "roughness": f32(mat.roughness),
        "nodes": nodes,
        "links": links,
    }


def dump_image(img):
    return {
        "name": img.name,
        "source": img.source,
        "alpha_mode": img.alpha_mode,
        "packed": img.packed_file is not None,
        "packed_size": img.packed_file.size if img.packed_file is not None else 0,
        "colorspace": img.colorspace_settings.name,
    }


def dump_camera(cam):
    return {
        "name": cam.name,
        "type": cam.type,
        "lens": f32(cam.lens),
        "sensor_fit": cam.sensor_fit,
        "lens_unit": cam.lens_unit,
        "ortho_scale": f32(cam.ortho_scale),
        "clip_start": f32(cam.clip_start),
        "clip_end": f32(cam.clip_end),
    }


def dump_light(la):
    d = {"name": la.name, "type": la.type, "energy": f32(la.energy), "color": fvec(la.color)}
    if la.type == "SPOT":
        d["spot_size"] = f32(la.spot_size)
        d["spot_blend"] = f32(la.spot_blend)
    return d


def dump():
    return {
        "blender": {
            "version": bpy.app.version_string,
            "build_hash": bpy.app.build_hash.decode("ascii", "replace"),
        },
        "window_scene": bpy.context.window.scene.name,
        "scenes": [dump_scene(sc) for sc in bpy.data.scenes],
        "collections": [
            {"name": c.name, "objects": [o.name for o in c.objects], "children": [ch.name for ch in c.children]}
            for c in bpy.data.collections
        ],
        "objects": [dump_object(ob) for ob in bpy.data.objects],
        "meshes": [dump_mesh(me) for me in bpy.data.meshes],
        "materials": [dump_material(m) for m in bpy.data.materials],
        "images": [dump_image(i) for i in bpy.data.images],
        "cameras": [dump_camera(c) for c in bpy.data.cameras],
        "lights": [dump_light(l) for l in bpy.data.lights],
    }


# --------------------------------------------------------------------------- main


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = "tests/data/gltf-blender"
    flt = None
    export = True
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out = argv[i + 1]
            i += 2
        elif argv[i] == "--filter":
            flt = argv[i + 1]
            i += 2
        elif argv[i] == "--no-export":
            export = False
            i += 1
        else:
            raise SystemExit("unknown argument %r" % argv[i])
    out = os.path.join(REPO, out) if not os.path.isabs(out) else out
    os.makedirs(os.path.join(out, "fixtures"), exist_ok=True)
    os.makedirs(os.path.join(out, "dumps"), exist_ok=True)

    if export:
        for name, build, kwargs in FIXTURES:
            if flt and flt not in name:
                continue
            build()
            path = os.path.join(out, "fixtures", name + ".glb")
            bpy.ops.export_scene.gltf(filepath=path, **kwargs)
            print("fixture", name)

    for case, src, settings in CASES:
        if flt and flt not in case:
            continue
        reset()
        if src.startswith(FIXTURES_DIR + "/"):
            # Fixtures are read from <out> (the committed ones with --no-export).
            path = os.path.join(out, "fixtures", src[len(FIXTURES_DIR) + 1:])
            if not os.path.isfile(path):
                path = os.path.join(REPO, src)
        else:
            path = os.path.join(REPO, src)
        bpy.ops.import_scene.gltf(filepath=path, **settings)
        state = dump()
        state["source"] = src
        state["settings"] = settings
        with open(os.path.join(out, "dumps", case + ".json"), "w", encoding="utf-8") as f:
            # One top-level key per line keeps diffs readable and the files small.
            f.write("{\n")
            items = list(state.items())
            for i, (k, v) in enumerate(items):
                f.write("%s:%s%s\n" % (json.dumps(k), json.dumps(v, separators=(",", ":")),
                                        "," if i + 1 < len(items) else ""))
            f.write("}\n")
        print("dump", case)


main()
