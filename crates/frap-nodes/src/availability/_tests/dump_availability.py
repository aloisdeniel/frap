# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump the socket `enabled` sets of every shader node type for every combination of its mode
properties (WP 3.1 acceptance: "socket enabled sets equal Blender per mode for every supported
node"). Runs INSIDE Blender 5.2.4:

    $BLENDER_BIN -b --factory-startup --python dump_availability.py -- availability.blender.json

For each node type (every `ShaderNode*` that `nodes.new` accepts, plus the generic group
input/output, frame and reroute nodes), the cartesian product of all its own writable
non-flag ENUM and BOOLEAN properties is applied to one node. The properties whose value
changes the enabled sets for some combination of the others are "relevant"; the output keeps
one case per combination of the relevant ones (the others at their defaults) as
`[values, input bits, output bits]`, a bit per socket in `inputs`/`outputs` order. The script
also checks that no mode changes the socket lists themselves and that the projection on the
relevant properties is consistent.
"""

import itertools
import json
import sys

import bpy

SKIP = {"rna_type", "inputs", "outputs", "internal_links", "dimensions", "location",
        "location_absolute", "select", "is_active_output", "parent", "warning_propagation",
        "interface", "name", "label", "width", "height", "hide", "mute", "show_options",
        "show_preview", "show_texture", "use_custom_color", "color", "color_tag", "type"}
GENERIC = ["NodeGroupInput", "NodeGroupOutput", "NodeFrame", "NodeReroute"]


def bits(socks):
    return "".join("1" if s.enabled else "0" for s in socks)


def node_types():
    names = []
    for n in dir(bpy.types):
        t = getattr(bpy.types, n)
        if n.startswith("ShaderNode") and n != "ShaderNode" and isinstance(t, type) \
                and issubclass(t, bpy.types.ShaderNode):
            names.append(n)
    return sorted(names) + GENERIC


def mode_props(node):
    base = node.bl_rna.base
    base_ids = set(base.properties.keys()) if base else set()
    out = []
    for p in node.bl_rna.properties:
        if p.identifier in SKIP or p.identifier in base_ids or p.identifier.startswith("bl_") \
                or p.is_readonly:
            continue
        if p.type == "ENUM" and not p.is_enum_flag:
            vals = [it.identifier for it in p.enum_items]
            if vals:
                out.append((p.identifier, vals))
        elif p.type == "BOOLEAN" and not p.is_array:
            out.append((p.identifier, [False, True]))
    return out


def dump_type(tree, idname):
    node = tree.nodes.new(idname)
    props = mode_props(node)
    keys = [k for k, _ in props]
    default_bits = [bits(node.inputs), bits(node.outputs)]
    ins = [s.identifier for s in node.inputs]
    outs = [s.identifier for s in node.outputs]
    combos = []
    for vals in itertools.product(*[v for _, v in props]):
        try:
            for k, v in zip(keys, vals):
                setattr(node, k, v)
        except Exception:  # noqa: BLE001  (an item the node refuses, e.g. Mix ROTATION)
            continue
        if [s.identifier for s in node.inputs] != ins or \
                [s.identifier for s in node.outputs] != outs:
            raise SystemExit(f"{idname}: socket list changed for {dict(zip(keys, vals))}")
        combos.append((vals, (bits(node.inputs), bits(node.outputs))))
    relevant = []
    for i in range(len(keys)):
        groups = {}
        for vals, e in combos:
            groups.setdefault(vals[:i] + vals[i + 1:], set()).add(e)
        if any(len(g) > 1 for g in groups.values()):
            relevant.append(i)
    table = {}
    for vals, e in combos:
        rk = tuple(vals[i] for i in relevant)
        if table.setdefault(rk, e) != e:
            raise SystemExit(f"{idname}: inconsistent projection on {rk}")
    tree.nodes.remove(node)
    return {
        "inputs": ins,
        "outputs": outs,
        "relevant": [keys[i] for i in relevant],
        "default": default_bits,
        "cases": [[list(rk), e[0], e[1]] for rk, e in table.items()],
    }


def main():
    out = sys.argv[sys.argv.index("--") + 1]
    tree = bpy.data.materials.new("DumpAvailability").node_tree
    for n in list(tree.nodes):
        tree.nodes.remove(n)
    res = {}
    for idname in node_types():
        try:
            tree.nodes.new(idname)
        except RuntimeError:
            continue  # abstract or removed type
        tree.nodes.remove(tree.nodes[-1])
        res[idname] = dump_type(tree, idname)
    data = {
        "blender": bpy.app.version_string,
        "build_hash": bpy.app.build_hash.decode(),
        "types": res,
    }
    with open(out, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1, sort_keys=True)
        f.write("\n")
    print(f"dump_availability.py: {len(res)} node types")


main()
