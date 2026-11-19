# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump the RNA reads of every shader node type, its sockets and the shader trees that WP 3.1
binds from constants (`rna/info.rs`, `rna.rs`). Runs INSIDE Blender 5.2.4:

    $BLENDER_BIN -b --factory-startup --python _dump_node_rna.py -- _node_rna.blender.json

Each node type is created with `nodes.new` in a fresh material; values are read on the fresh
node. `rna/tests.rs` (`node_rna_constants_equal_blender`) compares frap's reads.
"""

import json
import sys

import bpy

NODE_PROPS = ["type", "bl_static_type", "bl_label", "bl_description", "bl_icon", "bl_width_default",
              "bl_width_min", "bl_width_max", "bl_height_default", "bl_height_min", "bl_height_max",
              "color", "use_custom_color", "color_tag", "dimensions", "height", "show_options",
              "show_preview", "show_texture", "warning_propagation", "width", "hide", "mute", "label"]
SOCKET_PROPS = ["name", "identifier", "label", "description", "bl_label", "bl_subtype_label",
                "display_shape", "inferred_structure_type", "is_icon_visible", "pin_gizmo",
                "show_expanded", "hide", "link_limit", "is_multi_input", "enabled", "type", "bl_idname",
                "is_output", "is_unavailable", "hide_value"]
TREE_PROPS = ["name", "bl_idname", "type", "color_tag", "default_group_node_width", "description",
              "bl_label", "bl_description", "bl_icon", "bl_use_group_interface", "view_center",
              "annotation", "id_type", "is_evaluated", "is_missing", "is_runtime_data", "tag",
              "use_extra_user", "is_editable", "is_embedded_data", "asset_data", "preview",
              "override_library", "library_weak_reference", "is_library_indirect"]


def jv(v):
    if isinstance(v, float):
        return v
    if v is None or isinstance(v, (str, bool, int)):
        return v
    if isinstance(v, bpy.types.ID):
        return {"$id": v.name}
    try:
        return [jv(x) for x in v]
    except TypeError:
        return repr(v)


def read(obj, props):
    out = {}
    for p in props:
        try:
            out[p] = jv(getattr(obj, p))
        except Exception as e:  # noqa: BLE001
            out[p] = {"$error": f"{type(e).__name__}: {e}"}
    return out


def node_types():
    names = []
    for n in dir(bpy.types):
        t = getattr(bpy.types, n)
        if n.startswith("ShaderNode") and n != "ShaderNode" and isinstance(t, type) \
                and issubclass(t, bpy.types.ShaderNode):
            names.append(n)
    return sorted(names) + ["NodeGroupInput", "NodeGroupOutput", "NodeFrame", "NodeReroute"]


def main():
    out_path = sys.argv[sys.argv.index("--") + 1]
    tree = bpy.data.materials.new("DumpNodeRna").node_tree
    for n in list(tree.nodes):
        tree.nodes.remove(n)
    nodes = {}
    for idname in node_types():
        try:
            n = tree.nodes.new(idname)
        except RuntimeError:
            continue
        d = read(n, NODE_PROPS)
        d["inputs"] = [read(s, SOCKET_PROPS) for s in n.inputs]
        d["outputs"] = [read(s, SOCKET_PROPS) for s in n.outputs]
        nodes[idname] = d
        tree.nodes.remove(n)
    trees = {
        "material": read(bpy.data.materials["Material"].node_tree, TREE_PROPS),
        "world": read(bpy.data.worlds["World"].node_tree, TREE_PROPS),
        "group": read(bpy.data.node_groups.new("NodeGroup", "ShaderNodeTree"), TREE_PROPS),
    }
    data = {"blender": bpy.app.version_string, "build_hash": bpy.app.build_hash.decode(),
            "nodes": nodes, "trees": trees}
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1, sort_keys=True)
        f.write("\n")
    print(f"_dump_node_rna.py: {len(nodes)} node types")


main()
