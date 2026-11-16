# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender side of the mesh RNA fixture (WP 1.27).

Builds the test mesh with `Mesh.from_pydata`, applies the RNA writes listed in `WRITES`
(the Rust test applies the same writes through frap's bindings), then dumps every bound
property of every element collection, item by item and through `foreach_get`.

Run (regenerates `blender_reads.json` next to this file):

    $BLENDER_BIN -b --factory-startup --python crates/frap-mesh/src/rna/tests/dump_reads.py
"""
import json
import os

import bpy

VERTS = [(0, 0, 0), (1, 0, 0), (1, 1, 0.3), (0, 1, 0), (2, 0.5, 0.1), (0.5, 2, 0.2),
         (-1, 1.5, 0), (3, 3, 3)]
EDGES = [(7, 0)]
FACES = [(0, 1, 2, 3), (1, 4, 2), (3, 2, 4, 5, 6)]

ELEMENT_PROPS = {
    "vertices": ["co", "normal", "select", "hide", "index", "undeformed_co"],
    "edges": ["vertices", "select", "hide", "use_seam", "use_edge_sharp", "is_loose", "index"],
    "polygons": ["vertices", "loop_start", "loop_total", "material_index", "select", "hide",
                 "use_smooth", "normal", "center", "area", "index"],
    "loops": ["vertex_index", "edge_index", "index", "normal", "tangent", "bitangent_sign",
              "bitangent"],
    "loop_triangles": ["vertices", "loops", "polygon_index", "normal", "split_normals", "area",
                       "index", "material_index", "use_smooth"],
    "vertex_normals": ["vector"],
    "polygon_normals": ["vector"],
    "corner_normals": ["vector"],
    "loop_triangle_polygons": ["value"],
}

VALUE_FIELDS = {
    "FLOAT": ["value"], "INT": ["value"], "INT8": ["value"], "BOOLEAN": ["value"],
    "FLOAT2": ["vector"], "FLOAT_VECTOR": ["vector"], "FLOAT_COLOR": ["color", "color_srgb"],
    "BYTE_COLOR": ["color", "color_srgb"], "INT16_2D": ["value"], "INT32_2D": ["value"],
    "QUATERNION": ["value"], "FLOAT4X4": ["value"], "STRING": ["value"],
}


def plain(v):
    if isinstance(v, (bool, int, float, str)):
        return v
    if isinstance(v, bytes):
        return v.decode()
    if hasattr(v, "__len__"):
        flat = []
        for x in v:
            p = plain(x)
            flat.extend(p if isinstance(p, list) else [p])
        return flat
    return repr(v)


def foreach(coll, prop):
    rna = coll[0].bl_rna.properties[prop] if len(coll) else None
    if rna is None or rna.type not in {"FLOAT", "INT", "BOOLEAN"}:
        return None
    try:
        n = len(coll) * max(1, len(plain(getattr(coll[0], prop))) if rna.is_array else 1)
        buf = [0.0 if rna.type == "FLOAT" else 0] * n
        coll.foreach_get(prop, buf)
        return [plain(x) for x in buf]
    except RuntimeError as e:
        return "RuntimeError: " + str(e)


def dump_coll(coll, props):
    out = {}
    for p in props:
        out[p] = {"items": [plain(getattr(it, p)) for it in coll], "foreach": foreach(coll, p)}
    return out


bpy.ops.wm.read_factory_settings(use_empty=True)
me = bpy.data.meshes.new("T")
me.from_pydata(VERTS, EDGES, FACES)

# WRITES: keep in sync with `apply_writes` in fixture.rs.
me.vertices[1].select = False
me.vertices[2].hide = True
me.edges[0].use_seam = True
me.edges[1].use_edge_sharp = True
me.polygons[0].use_smooth = True
me.polygons[1].material_index = 2
me.vertices[7].co = (3, 2, 1)
me.attributes.new("w", "FLOAT", "POINT").data.foreach_set("value", [i * 0.5 for i in range(8)])
bc = me.attributes.new("bc", "BYTE_COLOR", "CORNER")
bc.data.foreach_set("color", [((i * 7) % 11) / 10.0 for i in range(12 * 4)])
fc = me.attributes.new("fc", "FLOAT_COLOR", "POINT")
fc.data.foreach_set("color", [((i * 3) % 7) / 4.0 for i in range(8 * 4)])
fc.data[1].color_srgb = (0.5, 0.25, 0.75, 1.0)
uv = me.attributes.new("UVMap", "FLOAT2", "CORNER")
uv.data.foreach_set("vector", [i * 0.125 for i in range(12 * 2)])
me.uv_layers["UVMap"].data[3].pin_uv = True
me.attributes.new("q", "QUATERNION", "FACE").data[1].value = (0.5, 0.5, 0.5, 0.5)
me.attributes.new("m4", "FLOAT4X4", "EDGE").data[0].value = (
    (1, 2, 3, 4), (5, 6, 7, 8), (9, 10, 11, 12), (13, 14, 15, 16))
me.attributes.new("s2", "INT16_2D", "CORNER").data[0].value = (3, -4)
me.attributes.new("i8", "INT8", "POINT").data[5].value = -7
me.attributes.new("w2", "FLOAT", "POINT")
me.attributes["w"].name = "w2"
# END WRITES

state = {
    "attributes": [[a.name, a.data_type, a.domain, a.is_internal, a.is_required,
                    type(a).__name__] for a in me.attributes],
    "color_attributes": [a.name for a in me.color_attributes],
    "uv_layers": [[u.name, u.active, u.active_render] for u in me.uv_layers],
    "vertex_colors": [[c.name, c.active, c.active_render] for c in me.vertex_colors],
    "mesh": {p: plain(getattr(me, p)) for p in [
        "normals_domain", "has_custom_normals", "texspace_location", "texspace_size",
        "use_auto_texspace", "total_vert_sel", "total_edge_sel", "total_face_sel"]},
    "elements": {c: dump_coll(getattr(me, c), ps) for c, ps in ELEMENT_PROPS.items()},
    "attribute_data": {
        a.name: dump_coll(a.data, VALUE_FIELDS[a.data_type])
        for a in me.attributes if a.data_type in VALUE_FIELDS
    },
    "uv_data": dump_coll(me.uv_layers["UVMap"].data, ["uv", "pin_uv"]),
    "uv_vectors": dump_coll(me.uv_layers["UVMap"].uv, ["vector"]),
    "vertex_color_data": {c.name: dump_coll(c.data, ["color"]) for c in me.vertex_colors},
}
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "blender_reads.json")
with open(out, "w") as f:
    json.dump(state, f, indent=1, sort_keys=True)
    f.write("\n")
print("wrote", out)
