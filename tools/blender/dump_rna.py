# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump Blender RNA struct definitions (bl_rna) to JSON, one file per struct.

Runs INSIDE Blender (ARCHITECTURE.md §4.1):

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_rna.py -- \
        --out spec/blender-5.2/ [--structs Object,Mesh,...] [--extra MaterialSlot,...] [--no-bases]

Writes <out>/rna/<Struct>.json and <out>/rna/_meta.json.

For every struct: identifier, name, description, base, name_property, nested,
own properties (base-chain properties are NOT repeated; lookup walks `base`),
functions (params / returns), and `fresh`: values read from a newly created
instance wherever they differ from the RNA default (ARCHITECTURE.md §2.1, §5.6).

The struct `Context` additionally gets `context_members`: members reachable via
`bpy.context.<name>` that are NOT RNA properties of `bpy.types.Context` (they come
from C context callbacks: object, active_object, selected_objects, ...). Their types
are inferred from live values inside a VIEW_3D override and from a few scene states.

First version written for WP 0.2 (contract probes). WP 1.1 extends it.
"""

import json
import math
import os
import sys

import bpy

# The WP 0.2 subset. WP 1.1 replaces this by "every struct reachable from bpy.types
# that frap may implement".
DEFAULT_STRUCTS = ["Object", "Mesh", "MeshVertex", "Material", "ShaderNodeTree", "NodeSocket"]
# Structs the M0 prototypes additionally need (refined socket type, derived slot, Context).
DEFAULT_EXTRA = ["MaterialSlot", "Node", "NodeSocketColor", "ShaderNodeBsdfPrincipled", "Context",
                 "Scene", "ViewLayer", "ToolSettings"]

FLAG_ATTRS = [
    # (bpy attribute, flag name in the dump)
    ("is_readonly", "READONLY"),
    ("is_hidden", "HIDDEN"),
    ("is_skip_save", "SKIP_SAVE"),
    ("is_enum_flag", "ENUM_FLAG"),
    ("is_required", "REQUIRED"),
    ("is_output", "OUTPUT"),
    ("is_animatable", "ANIMATABLE"),
    ("is_never_none", "NEVER_NULL"),
    ("is_argument_optional", "ARGUMENT_OPTIONAL"),
    ("is_library_editable", "LIBRARY_EDITABLE"),
    ("is_overridable", "OVERRIDABLE"),
    ("is_skip_preset", "SKIP_PRESET"),
    ("is_path_output", "PATH_OUTPUT"),
    ("is_path_supports_blend_relative", "PATH_SUPPORTS_BLEND_RELATIVE"),
    ("is_path_supports_templates", "PATH_SUPPORTS_TEMPLATES"),
    ("is_deprecated", "DEPRECATED"),
    ("is_runtime", "RUNTIME"),
    ("is_registered", "REGISTERED"),
    ("is_registered_optional", "REGISTERED_OPTIONAL"),
]


# ----------------------------------------------------------------------------- helpers

def jfloat(x):
    """JSON-safe float. Non-finite values use the protocol's tagged form (§13.3)."""
    x = float(x)
    if math.isnan(x):
        return {"$float": "nan"}
    if math.isinf(x):
        return {"$float": "inf" if x > 0 else "-inf"}
    return x


def jvalue(v):
    """Convert a live Python/mathutils/bpy value to JSON (for fresh-instance values)."""
    if v is None or isinstance(v, (bool, int, str)):
        return v
    if isinstance(v, float):
        return jfloat(v)
    if isinstance(v, (set, frozenset)):
        return sorted(v)
    if isinstance(v, bpy.types.ID):
        return {"$ref": repr(v)}
    if isinstance(v, bpy.types.bpy_struct):
        return {"$ptr": repr(v), "type": v.bl_rna.identifier}
    if hasattr(v, "__len__") and hasattr(v, "__getitem__"):
        try:
            return [jvalue(x) for x in v]
        except Exception:
            pass
    return {"$repr": repr(v)}


def array_shape(p):
    """[] for scalars, [n] for 1-D, [r, c] for 2-D (bound_box (8,3), matrix_world (4,4))."""
    if p.type not in ("BOOLEAN", "INT", "FLOAT"):
        return []
    dims = [d for d in p.array_dimensions if d]
    if not getattr(p, "is_array", False) and not dims:
        return []
    if getattr(p, "is_array", False) and not dims:
        return ["dynamic"]
    return dims


def enum_items(items):
    out = []
    for it in items:
        out.append({
            "identifier": it.identifier,
            "name": it.name,
            "description": it.description,
            "icon": it.icon,
            "value": it.value,
        })
    return out


def prop_to_json(p):
    d = {
        "identifier": p.identifier,
        "name": p.name,
        "description": p.description,
        "type": p.type,
        "subtype": p.subtype,
        "unit": p.unit,
    }
    shape = array_shape(p)
    if shape:
        d["array"] = shape
    if p.type in ("BOOLEAN", "INT", "FLOAT"):
        if shape:
            dflt = list(p.default_array)
            d["default"] = [jfloat(x) if p.type == "FLOAT" else x for x in dflt]
        else:
            d["default"] = jfloat(p.default) if p.type == "FLOAT" else p.default
        if p.type in ("INT", "FLOAT"):
            conv = jfloat if p.type == "FLOAT" else int
            d["hard_min"] = conv(p.hard_min)
            d["hard_max"] = conv(p.hard_max)
            d["soft_min"] = conv(p.soft_min)
            d["soft_max"] = conv(p.soft_max)
            d["step"] = jfloat(p.step) if p.type == "FLOAT" else p.step
        if p.type == "FLOAT":
            d["precision"] = p.precision
    elif p.type == "STRING":
        d["default"] = p.default
        d["length_max"] = p.length_max          # includes the NUL byte; 0 = unlimited
    elif p.type == "ENUM":
        d["items"] = enum_items(p.enum_items)
        static = enum_items(p.enum_items_static)
        if [i["identifier"] for i in static] != [i["identifier"] for i in d["items"]]:
            d["items_static"] = static
        if p.is_enum_flag:
            d["default"] = sorted(p.default_flag)
        else:
            d["default"] = p.default
    elif p.type == "POINTER":
        d["fixed_type"] = p.fixed_type.identifier if p.fixed_type else None
        d["is_id"] = bool(p.fixed_type and is_id_struct(p.fixed_type))
    elif p.type == "COLLECTION":
        d["fixed_type"] = p.fixed_type.identifier if p.fixed_type else None
        d["is_id"] = bool(p.fixed_type and is_id_struct(p.fixed_type))
        d["api"] = p.srna.identifier if p.srna else None
    d["flags"] = [name for attr, name in FLAG_ATTRS if getattr(p, attr, False)]
    if getattr(p, "is_deprecated", False):
        d["deprecated"] = {
            "note": p.deprecated_note,
            "version": list(p.deprecated_version),
            "removal_version": list(p.deprecated_removal_version),
        }
    tags = sorted(getattr(p, "tags", set()) or [])
    if tags:
        d["tags"] = tags
    return d


def is_id_struct(srna):
    s = srna
    while s is not None:
        if s.identifier == "ID":
            return True
        s = s.base
    return False


def func_to_json(f):
    params, returns = [], []
    for p in f.parameters:
        (returns if p.is_output else params).append(prop_to_json(p))
    return {
        "identifier": f.identifier,
        "description": f.description,
        "use_self": f.use_self,
        "use_self_type": f.use_self_type,
        "is_registered": f.is_registered,
        "is_registered_optional": f.is_registered_optional,
        "params": params,
        "returns": returns,
    }


def own_props(srna):
    base_ids = set(srna.base.properties.keys()) if srna.base else set()
    return [p for p in srna.properties if p.identifier != "rna_type" and p.identifier not in base_ids]


def own_funcs(srna):
    base_ids = set(srna.base.functions.keys()) if srna.base else set()
    return [f for f in srna.functions if f.identifier not in base_ids]


def struct_to_json(srna):
    return {
        "identifier": srna.identifier,
        "name": srna.name,
        "description": srna.description,
        "base": srna.base.identifier if srna.base else None,
        "name_property": srna.name_property.identifier if srna.name_property else None,
        "nested": srna.nested.identifier if srna.nested else None,
        "is_id": is_id_struct(srna),
        "props": [prop_to_json(p) for p in own_props(srna)],
        "funcs": [func_to_json(f) for f in own_funcs(srna)],
    }


# ----------------------------------------------------------------- fresh-instance values

def _principled(mat):
    return mat.node_tree.nodes["Principled BSDF"]


def fresh_instances():
    """Recipes creating a fresh instance per struct (the creation path is recorded).
    Each returns (recipe_text, instance)."""
    D = bpy.data
    def obj():
        return "bpy.data.objects.new('Object', None)", D.objects.new("Object", None)
    def mesh():
        return "bpy.data.meshes.new('Mesh')", D.meshes.new("Mesh")
    def mat():
        return "bpy.data.materials.new('Material')", D.materials.new("Material")
    def ntree():
        m = D.materials.new("FreshNT")
        return "bpy.data.materials.new('Material').node_tree", m.node_tree
    def vert():
        me = D.meshes.new("FreshVert")
        me.vertices.add(1)
        return "me = bpy.data.meshes.new('Mesh'); me.vertices.add(1); me.vertices[0]", me.vertices[0]
    def node():
        m = D.materials.new("FreshNode")
        return "bpy.data.materials.new('Material').node_tree.nodes['Principled BSDF']", _principled(m)
    def sock():
        m = D.materials.new("FreshSock")
        return "bpy.data.materials.new('Material').node_tree.nodes['Principled BSDF'].inputs[0]", _principled(m).inputs[0]
    def slot():
        me = D.meshes.new("FreshSlot")
        me.materials.append(D.materials.new("FreshSlotMat"))
        ob = D.objects.new("FreshSlot", me)
        return ("me = bpy.data.meshes.new('Mesh'); me.materials.append(bpy.data.materials.new('M')); "
                "bpy.data.objects.new('O', me).material_slots[0]"), ob.material_slots[0]
    return {
        "Object": obj, "Mesh": mesh, "Material": mat, "ShaderNodeTree": ntree,
        "MeshVertex": vert, "Node": node, "ShaderNodeBsdfPrincipled": node,
        "NodeSocket": sock, "NodeSocketColor": sock, "MaterialSlot": slot,
    }


def rna_default_json(p):
    if p.type in ("BOOLEAN", "INT", "FLOAT"):
        if array_shape(p):
            return [jfloat(x) if p.type == "FLOAT" else x for x in p.default_array]
        return jfloat(p.default) if p.type == "FLOAT" else p.default
    if p.type == "STRING":
        return p.default
    if p.type == "ENUM":
        return sorted(p.default_flag) if p.is_enum_flag else p.default
    return None


def flatten(v):
    if isinstance(v, list):
        out = []
        for x in v:
            out.extend(flatten(x))
        return out
    return [v]


# Per-instance identity, not a "fresh value" (would make every dump differ run to run).
IDENTITY_PROPS = {"rna_type", "name", "name_full", "session_uid"}


def fresh_diff(srna, inst):
    """Values on the fresh instance that differ from the RNA default (scalars/arrays/enums/strings)."""
    diff = {}
    for p in srna.properties:
        if p.identifier in IDENTITY_PROPS or p.type in ("POINTER", "COLLECTION"):
            continue
        try:
            live = jvalue(getattr(inst, p.identifier))
        except Exception as e:  # noqa: BLE001 - recorded, not fatal
            diff[p.identifier] = {"$error": f"{type(e).__name__}: {e}"}
            continue
        dflt = rna_default_json(p)
        if isinstance(live, list) and isinstance(dflt, list):
            if flatten(live) != flatten(dflt):
                diff[p.identifier] = live
        elif live != dflt:
            diff[p.identifier] = live
    return diff


# ------------------------------------------------------------------ context members

def context_members():
    """Members of bpy.context that are not RNA properties of bpy.types.Context.
    Types are inferred from live values in several states (inside a VIEW_3D override)."""
    rna_props = set(bpy.types.Context.bl_rna.properties.keys())
    seen = {}

    def classify(name, v):
        if name in rna_props or name.startswith("_"):
            return
        entry = seen.setdefault(name, {"identifier": name, "kinds": set(), "types": set()})
        if v is None:
            entry["kinds"].add("pointer_or_none")
        elif isinstance(v, (list, tuple)):
            entry["kinds"].add("collection")
            for x in v:
                entry["types"].add(type(x).bl_rna.identifier if hasattr(type(x), "bl_rna") else type(x).__name__)
        elif isinstance(v, bpy.types.bpy_struct):
            entry["kinds"].add("pointer")
            entry["types"].add(v.bl_rna.identifier)
        elif isinstance(v, str):
            entry["kinds"].add("string")
        else:
            entry["kinds"].add(type(v).__name__)

    def snapshot():
        win = bpy.context.window
        area = next(a for a in win.screen.areas if a.type == "VIEW_3D")
        region = next(r for r in area.regions if r.type == "WINDOW")
        with bpy.context.temp_override(window=win, area=area, region=region):
            for k, v in bpy.context.copy().items():
                classify(k, v)

    snapshot()                                  # factory: Cube active + selected
    bpy.ops.object.armature_add()
    bpy.ops.object.mode_set(mode="POSE")
    snapshot()                                  # pose mode
    bpy.ops.object.mode_set(mode="EDIT")
    snapshot()                                  # armature edit mode
    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.wm.read_factory_settings()
    bpy.ops.object.mode_set(mode="EDIT")
    snapshot()                                  # mesh edit mode
    bpy.ops.wm.read_factory_settings()

    out = []
    for name in sorted(seen):
        e = seen[name]
        kinds = e["kinds"] - {"pointer_or_none"}
        out.append({
            "identifier": name,
            "kind": "collection" if "collection" in kinds else ("pointer" if "pointer" in kinds else
                     (sorted(kinds)[0] if kinds else "unknown")),
            "types": sorted(e["types"]),
            "seen_none": "pointer_or_none" in e["kinds"],
        })
    return out


# ----------------------------------------------------------------------------- main

def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    args = {"out": None, "structs": DEFAULT_STRUCTS, "extra": DEFAULT_EXTRA, "bases": True}
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--out":
            args["out"] = argv[i + 1]; i += 2
        elif a == "--structs":
            args["structs"] = [s for s in argv[i + 1].split(",") if s]; i += 2
        elif a == "--extra":
            args["extra"] = [s for s in argv[i + 1].split(",") if s]; i += 2
        elif a == "--no-bases":
            args["bases"] = False; i += 1
        else:
            raise SystemExit(f"dump_rna.py: unknown argument {a!r}")
    if not args["out"]:
        raise SystemExit("dump_rna.py: --out DIR is required")
    return args


def enable_addon_prefs():
    # §4.1: without this, glTF variant RNA is not registered.
    try:
        bpy.context.preferences.addons["io_scene_gltf2"].preferences.KHR_materials_variants_ui = True
    except Exception as e:  # noqa: BLE001
        print(f"dump_rna.py: warning: cannot enable KHR_materials_variants_ui: {e}", file=sys.stderr)


def blender_meta():
    return {
        "blender_version": bpy.app.version_string,
        "build_hash": bpy.app.build_hash.decode() if isinstance(bpy.app.build_hash, bytes) else bpy.app.build_hash,
        "factory_startup": True,
        "property_types": [i.identifier for i in bpy.types.Property.bl_rna.properties["type"].enum_items],
        "subtypes": [i.identifier for i in bpy.types.Property.bl_rna.properties["subtype"].enum_items],
        "units": [i.identifier for i in bpy.types.Property.bl_rna.properties["unit"].enum_items],
    }


def write_json(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1, ensure_ascii=False, allow_nan=False, sort_keys=False)
        f.write("\n")


def main():
    args = parse_args()
    enable_addon_prefs()
    names = list(dict.fromkeys(args["structs"] + args["extra"]))
    if args["bases"]:
        closure = []
        for n in names:
            s = getattr(bpy.types, n).bl_rna
            chain = []
            while s is not None:
                chain.append(s.identifier)
                s = s.base
            for c in reversed(chain):
                if c not in closure:
                    closure.append(c)
        names = closure

    recipes = fresh_instances()
    out_dir = os.path.join(args["out"], "rna")
    for n in names:
        srna = getattr(bpy.types, n).bl_rna
        data = struct_to_json(srna)
        data["requested"] = n in args["structs"]
        if n in recipes:
            recipe, inst = recipes[n]()
            data["fresh"] = {"recipe": recipe, "differs_from_default": fresh_diff(srna, inst)}
        if n == "Context":
            data["context_members"] = context_members()
            enable_addon_prefs()
        write_json(os.path.join(out_dir, n + ".json"), data)
        print(f"dump_rna.py: {n}: {len(data['props'])} props, {len(data['funcs'])} funcs")
    meta = blender_meta()
    meta["structs"] = names
    write_json(os.path.join(out_dir, "_meta.json"), meta)


if __name__ == "__main__":
    main()
