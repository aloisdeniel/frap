# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump Blender operator definitions and poll matrices to JSON, one file per namespace.

Runs INSIDE Blender (ARCHITECTURE.md §4.1):

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_ops.py -- \
        --out spec/blender-5.2/ [--namespaces mesh,object]

Writes <out>/ops/<namespace>.json and <out>/ops/_meta.json.

Per operator: idname, bl_idname, label, description, options (bl_options), every property
(same property schema as dump_rna.py), macro steps (POINTER props whose fixed_type is an
operator type, in order), and the poll matrix:

    A  object mode, factory scene (mesh 'Cube' active and selected)
    B  object mode, nothing selected, no active object
    C  mesh edit mode (Cube)
    D  armature pose mode (a new armature, active and selected)
    E  armature edit mode
    each with and without a VIEW_3D window/area/region override ("A" vs "AV").

Cell values: "Y" poll() True, "N" poll() False, "X:<exception>" poll() raised.
Enum items that differ between contexts are recorded under "items_by_context" and the
property gets "dynamic_hint": true (a heuristic: Blender does not expose a dynamic flag).

The glTF add-on preference `KHR_materials_variants_ui` is switched on before dumping (§4.1,
shared with dump_rna.py) and recorded in `_meta.json` under `addon_prefs`.

First version written for WP 0.2 (contract probes); WP 1.1 extended it to the curated
namespace list of §4.1.
"""

import json
import os
import sys

import bpy

# No __pycache__/ next to the scripts (it would end up untracked in the repo).
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dump_rna  # noqa: E402  (shared property serialiser; same JSON schema)

# The curated namespace list of ARCHITECTURE §4.1: every operator in these is dumped.
DEFAULT_NAMESPACES = [
    "object", "mesh", "uv", "transform", "view3d", "anim", "screen", "render", "wm", "material",
    "image", "world", "collection", "scene", "geometry", "armature", "pose", "nla", "export_scene",
    "import_scene",
]
CONTEXTS = ["A", "B", "C", "D", "E"]


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    args = {"out": None, "namespaces": DEFAULT_NAMESPACES}
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--out":
            args["out"] = argv[i + 1]; i += 2
        elif a == "--namespaces":
            args["namespaces"] = [s for s in argv[i + 1].split(",") if s]; i += 2
        else:
            raise SystemExit(f"dump_ops.py: unknown argument {a!r}")
    if not args["out"]:
        raise SystemExit("dump_ops.py: --out DIR is required")
    return args


# --------------------------------------------------------------------------- contexts

def reset():
    bpy.ops.wm.read_factory_settings()
    dump_rna.enable_addon_prefs()


def setup(ctx):
    reset()
    if ctx == "A":
        pass
    elif ctx == "B":
        bpy.ops.object.select_all(action="DESELECT")
        bpy.context.view_layer.objects.active = None
    elif ctx == "C":
        bpy.ops.object.mode_set(mode="EDIT")
    elif ctx == "D":
        bpy.ops.object.armature_add()
        bpy.ops.object.mode_set(mode="POSE")
    elif ctx == "E":
        bpy.ops.object.armature_add()
        bpy.ops.object.mode_set(mode="EDIT")
    else:
        raise ValueError(ctx)
    expect = {"A": "OBJECT", "B": "OBJECT", "C": "EDIT_MESH", "D": "POSE", "E": "EDIT_ARMATURE"}[ctx]
    assert bpy.context.mode == expect, (ctx, bpy.context.mode)


def view3d_override():
    win = bpy.context.window
    area = next(a for a in win.screen.areas if a.type == "VIEW_3D")
    region = next(r for r in area.regions if r.type == "WINDOW")
    return bpy.context.temp_override(window=win, area=area, region=region)


def poll_one(op):
    try:
        return "Y" if op.poll() else "N"
    except Exception as e:  # noqa: BLE001
        return f"X:{type(e).__name__}: {e}"


def enum_ids(rna):
    out = {}
    for p in rna.properties:
        if p.type == "ENUM":
            try:
                out[p.identifier] = [i.identifier for i in p.enum_items]
            except Exception as e:  # noqa: BLE001
                out[p.identifier] = [f"$error:{e}"]
    return out


# ---------------------------------------------------------------------------- dumping

def op_list(ns):
    mod = getattr(bpy.ops, ns)
    return [(name, getattr(mod, name)) for name in sorted(dir(mod)) if not name.startswith("_")]


def op_static(ns, name, op):
    rna = op.get_rna_type()
    props = [p for p in rna.properties if p.identifier != "rna_type"]
    macro_steps = [p.fixed_type.identifier for p in props
                   if p.type == "POINTER" and p.fixed_type is not None and "_OT_" in p.fixed_type.identifier]
    return {
        "idname": f"{ns}.{name}",
        "bl_idname": op.idname(),
        "label": rna.name,
        "description": rna.description,
        "options": sorted(op.bl_options),
        "macro_steps": macro_steps,
        "props": [dump_rna.prop_to_json(p) for p in props],
    }


def main():
    args = parse_args()
    reset()
    ops = {}
    for ns in args["namespaces"]:
        for name, op in op_list(ns):
            try:
                ops[f"{ns}.{name}"] = (ns, op, op_static(ns, name, op))
            except Exception as e:  # noqa: BLE001
                print(f"dump_ops.py: {ns}.{name}: {e}", file=sys.stderr)

    # Poll matrix and per-context enum items.
    enum_by_ctx = {k: {} for k in ops}
    for ctx in CONTEXTS:
        setup(ctx)
        for key, (_ns, op, data) in ops.items():
            data.setdefault("poll", {})[ctx] = poll_one(op)
            enum_by_ctx[key][ctx] = enum_ids(op.get_rna_type())
        with view3d_override():
            for key, (_ns, op, data) in ops.items():
                data["poll"][ctx + "V"] = poll_one(op)
                enum_by_ctx[key][ctx + "V"] = enum_ids(op.get_rna_type())
    reset()

    for key, (_ns, _op, data) in ops.items():
        per = enum_by_ctx[key]
        for p in data["props"]:
            if p["type"] != "ENUM":
                continue
            seqs = {c: per[c].get(p["identifier"]) for c in per}
            distinct = {tuple(v) for v in seqs.values() if v is not None}
            if len(distinct) > 1 or "items_static" in p:
                p["dynamic_hint"] = True
                p["items_by_context"] = seqs

    by_ns = {}
    for key, (ns, _op, data) in ops.items():
        by_ns.setdefault(ns, []).append(data)
    out_dir = os.path.join(args["out"], "ops")
    for ns, lst in by_ns.items():
        lst.sort(key=lambda d: d["idname"])
        dump_rna.write_json(os.path.join(out_dir, ns + ".json"),
                            {"namespace": ns, "count": len(lst), "operators": lst})
        print(f"dump_ops.py: {ns}: {len(lst)} operators")
    meta = dump_rna.blender_meta()
    meta["namespaces"] = args["namespaces"]
    meta["poll_contexts"] = {
        "A": "object mode, factory scene (Cube active and selected)",
        "B": "object mode, nothing selected, no active object",
        "C": "mesh edit mode (Cube)",
        "D": "armature pose mode (new armature active and selected)",
        "E": "armature edit mode",
        "<X>V": "context <X> inside temp_override(window, area=VIEW_3D, region=WINDOW)",
    }
    dump_rna.write_json(os.path.join(out_dir, "_meta.json"), meta)


if __name__ == "__main__":
    main()
