# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump shader node types (sockets, defaults, availability) to JSON, one file per bl_idname.

Runs INSIDE Blender (ARCHITECTURE.md §4.1):

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_nodes.py -- --out spec/blender-5.2/

Writes <out>/nodes/<bl_idname>.json and <out>/nodes/_meta.json.

Every `ShaderNode*` type (plus the generic NodeGroupInput/NodeGroupOutput/NodeFrame/NodeReroute)
is created with `material.node_tree.nodes.new(type)` in a fresh material. Per node type:

* `bl_idname`, `name` (the default node name), `label`, `width`, `height`, `hide`;
* `inputs` / `outputs`: `name`, `identifier`, socket `bl_idname`, `type`, `default_value`
  (absent for sockets without one), `enabled`, `hide`, `hide_value`;
* `props`: the node's own RNA property values on the fresh node (POINTER values as `$ref`);
* `availability`: for every own non-flag ENUM property, each item value -> the identifiers of
  the enabled inputs and outputs after setting it (other props at their defaults). This is what
  `frap-nodes/src/availability/<node>.rs` reproduces (WP 1.11 / 3.1 tests).

Types that cannot be instantiated (abstract bases, removed 4.x types) are listed in `_meta.json`
under `not_instantiable` with the error text.
"""

import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dump_rna  # noqa: E402  (shared JSON helpers)

GENERIC = ["NodeGroupInput", "NodeGroupOutput", "NodeFrame", "NodeReroute"]
SKIP_PROPS = {"rna_type", "inputs", "outputs", "internal_links", "dimensions", "location",
              "location_absolute", "select", "is_active_output", "parent", "warning_propagation",
              "interface", "bl_idname", "bl_label", "bl_description", "bl_icon", "bl_static_type",
              "bl_width_default", "bl_width_min", "bl_width_max", "bl_height_default",
              "bl_height_min", "bl_height_max", "type", "name", "label", "width", "height", "hide",
              "mute", "show_options", "show_preview", "show_texture", "use_custom_color",
              "color", "color_tag", "debug_zone_body_lazy_function_graph",
              "debug_zone_lazy_function_graph"}


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = None
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out = argv[i + 1]; i += 2
        else:
            raise SystemExit(f"dump_nodes.py: unknown argument {argv[i]!r}")
    if not out:
        raise SystemExit("dump_nodes.py: --out DIR is required")
    return out


def node_types():
    names = [n for n in dump_rna.subclasses_of("ShaderNode")]
    return sorted(set(names)) + GENERIC


def socket_json(s):
    d = {
        "name": s.name,
        "identifier": s.identifier,
        "bl_idname": s.bl_idname,
        "type": s.type,
        "enabled": bool(s.enabled),
        "hide": bool(s.hide),
        "hide_value": bool(s.hide_value),
    }
    if "default_value" in s.bl_rna.properties:
        d["default_value"] = dump_rna.jvalue(s.default_value)
        p = s.bl_rna.properties["default_value"]
        d["default_value_type"] = p.type
        d["default_value_subtype"] = p.subtype
        if p.type in ("FLOAT", "INT"):
            d["min"] = dump_rna.jfloat(p.hard_min) if p.type == "FLOAT" else p.hard_min
            d["max"] = dump_rna.jfloat(p.hard_max) if p.type == "FLOAT" else p.hard_max
    return d


def enabled_ids(node):
    return {
        "inputs": [s.identifier for s in node.inputs if s.enabled],
        "outputs": [s.identifier for s in node.outputs if s.enabled],
    }


def own_enum_props(node):
    base = node.bl_rna.base
    base_ids = set(base.properties.keys()) if base else set()
    for p in node.bl_rna.properties:
        if p.identifier in SKIP_PROPS or p.identifier in base_ids:
            continue
        if p.type == "ENUM" and not p.is_enum_flag and not p.is_readonly:
            yield p


def prop_values(node):
    out = {}
    base = node.bl_rna
    for p in base.properties:
        if p.identifier in SKIP_PROPS or p.type == "COLLECTION":
            continue
        if p.identifier.startswith("bl_"):
            continue
        try:
            out[p.identifier] = dump_rna.jvalue(getattr(node, p.identifier))
        except Exception as e:  # noqa: BLE001
            out[p.identifier] = {"$error": f"{type(e).__name__}: {e}"}
    return out


def dump_node(tree, idname):
    node = tree.nodes.new(idname)
    data = {
        "bl_idname": node.bl_idname,
        "srna": node.bl_rna.identifier,
        "name": node.name,
        "label": node.label,
        "width": dump_rna.jfloat(node.width),
        "height": dump_rna.jfloat(node.height),
        "hide": bool(node.hide),
        "inputs": [socket_json(s) for s in node.inputs],
        "outputs": [socket_json(s) for s in node.outputs],
        "props": prop_values(node),
    }
    avail = {}
    for p in own_enum_props(node):
        default = getattr(node, p.identifier)
        per = {}
        for it in p.enum_items:
            try:
                setattr(node, p.identifier, it.identifier)
            except Exception as e:  # noqa: BLE001
                per[it.identifier] = {"$error": f"{type(e).__name__}: {e}"}
                continue
            per[it.identifier] = enabled_ids(node)
        try:
            setattr(node, p.identifier, default)
        except Exception:  # noqa: BLE001
            pass
        avail[p.identifier] = {"default": default, "by_value": per}
    if avail:
        data["availability"] = avail
    tree.nodes.remove(node)
    return data


def main():
    out = parse_args()
    dump_rna.enable_addon_prefs()
    mat = bpy.data.materials.new("DumpNodes")
    tree = mat.node_tree
    for n in list(tree.nodes):
        tree.nodes.remove(n)
    out_dir = os.path.join(out, "nodes")
    os.makedirs(out_dir, exist_ok=True)
    for f in os.listdir(out_dir):
        if f.endswith(".json"):
            os.remove(os.path.join(out_dir, f))
    done, failed = [], {}
    for idname in node_types():
        try:
            data = dump_node(tree, idname)
        except Exception as e:  # noqa: BLE001
            failed[idname] = f"{type(e).__name__}: {e}"
            continue
        dump_rna.write_json(os.path.join(out_dir, idname + ".json"), data)
        done.append(idname)
    meta = dump_rna.blender_meta()
    meta["node_types"] = done
    meta["not_instantiable"] = failed
    dump_rna.write_json(os.path.join(out_dir, "_meta.json"), meta)
    print(f"dump_nodes.py: {len(done)} node types, {len(failed)} not instantiable")


if __name__ == "__main__":
    main()
