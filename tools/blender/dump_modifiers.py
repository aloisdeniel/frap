# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump modifier types: fresh values from `modifiers.new` and default names from `modifier_add`.

Runs INSIDE Blender (ARCHITECTURE.md §4.1, [RNA §5.1]):

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_modifiers.py -- --out spec/blender-5.2/

Writes <out>/modifiers/<TYPE>.json and <out>/modifiers/_meta.json.

For every item of `Modifier.type` the factory Cube gets one modifier through
`cube.modifiers.new(name="M", type=TYPE)`; the dump records the refined struct (`srna`) and the
value of every property of that struct (base `Modifier` props included; POINTER values as
`$ref`/`$ptr`, COLLECTION props as their length). `default_name` is the name
`bpy.ops.object.modifier_add(type=TYPE)` gives on the same Cube. Types a mesh object cannot
take (grease-pencil modifiers) are listed in `_meta.json` under `not_for_mesh`.
"""

import os
import sys

import bpy

# No __pycache__/ next to the scripts (it would end up untracked in the repo).
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dump_rna  # noqa: E402  (shared JSON helpers)

SKIP = {"rna_type"}


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = None
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out = argv[i + 1]; i += 2
        else:
            raise SystemExit(f"dump_modifiers.py: unknown argument {argv[i]!r}")
    if not out:
        raise SystemExit("dump_modifiers.py: --out DIR is required")
    return out


def stable(v):
    """Session temp paths (OCEAN `filepath`) differ per run: written as `$TEMPDIR/...`."""
    if isinstance(v, str) and bpy.app.tempdir and v.startswith(bpy.app.tempdir):
        return "$TEMPDIR/" + v[len(bpy.app.tempdir):]
    return v


def values(md):
    out = {}
    for p in md.bl_rna.properties:
        if p.identifier in SKIP:
            continue
        try:
            v = getattr(md, p.identifier)
        except Exception as e:  # noqa: BLE001
            out[p.identifier] = {"$error": f"{type(e).__name__}: {e}"}
            continue
        if p.type == "COLLECTION":
            out[p.identifier] = {"$len": len(v)}
        else:
            out[p.identifier] = stable(dump_rna.jvalue(v))
    return out


def main():
    out = parse_args()
    dump_rna.enable_addon_prefs()
    cube = bpy.data.objects["Cube"]
    bpy.context.view_layer.objects.active = cube
    types = [(i.identifier, i.name) for i in bpy.types.Modifier.bl_rna.properties["type"].enum_items]
    out_dir = os.path.join(out, "modifiers")
    os.makedirs(out_dir, exist_ok=True)
    for f in os.listdir(out_dir):
        if f.endswith(".json"):
            os.remove(os.path.join(out_dir, f))
    done, failed = [], {}
    for t, ui_name in types:
        try:
            md = cube.modifiers.new(name="M", type=t)
        except Exception as e:  # noqa: BLE001
            failed[t] = f"{type(e).__name__}: {e}"
            continue
        if md is None:
            failed[t] = "modifiers.new returned None"
            continue
        data = {
            "type": t,
            "ui_name": ui_name,
            "srna": md.bl_rna.identifier,
            "values": values(md),
        }
        cube.modifiers.remove(md)
        try:
            bpy.ops.object.modifier_add(type=t)
            added = cube.modifiers[-1]
            data["default_name"] = added.name
            cube.modifiers.remove(added)
        except Exception as e:  # noqa: BLE001
            data["default_name"] = None
            data["modifier_add_error"] = f"{type(e).__name__}: {e}"
        dump_rna.write_json(os.path.join(out_dir, t + ".json"), data)
        done.append(t)
    meta = dump_rna.blender_meta()
    meta["types"] = done
    meta["not_for_mesh"] = failed
    dump_rna.write_json(os.path.join(out_dir, "_meta.json"), meta)
    print(f"dump_modifiers.py: {len(done)} types, {len(failed)} not for meshes")


if __name__ == "__main__":
    main()
