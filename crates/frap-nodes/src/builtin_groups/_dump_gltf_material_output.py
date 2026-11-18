# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump the glTF add-on's "glTF Material Output" group and a group node using it (WP 3.1).
Runs INSIDE Blender 5.2.4:

    $BLENDER_BIN -b --factory-startup --python _dump_gltf_material_output.py -- \\
        _gltf_material_output.blender.json

Calls the add-on's own `create_settings_group(get_gltf_node_name())`, then dumps the group
(interface items, nodes and sockets, links), a fresh `ShaderNodeGroup` and one whose
`node_tree` is the group, user counts, and the name of a second creation.
`gltf_material_output.rs` compares `new_group()` and `NodeTree::add_group_node` with it.
"""

import json
import sys

import bpy
from io_scene_gltf2.blender.com.material_helpers import create_settings_group, get_gltf_node_name


def jv(v):
    try:
        return [round(x, 6) for x in v]
    except TypeError:
        return v if not isinstance(v, float) else round(v, 6)


def sock(s):
    d = {k: getattr(s, k) for k in ("name", "identifier", "type", "bl_idname", "enabled", "hide",
                                    "hide_value", "is_multi_input", "link_limit", "label",
                                    "description")}
    if hasattr(s, "default_value"):
        d["default_value"] = jv(s.default_value)
    return d


def node(n):
    return {"name": n.name, "bl_idname": n.bl_idname, "label": n.label, "location": jv(n.location),
            "width": n.width, "inputs": [sock(s) for s in n.inputs],
            "outputs": [sock(s) for s in n.outputs],
            "node_tree": n.node_tree.name if getattr(n, "node_tree", None) else None}


def item(it):
    d = {"item_type": it.item_type, "name": it.name}
    for k in ("identifier", "in_out", "socket_type", "bl_socket_idname", "description", "subtype",
              "default_value", "min_value", "max_value", "hide_value", "hide_in_modifier",
              "force_non_field", "default_attribute_name", "index", "position", "attribute_domain",
              "default_input", "structure_type", "is_inspect_output", "is_panel_toggle",
              "layer_selection_field", "optional_label", "menu_expanded"):
        if hasattr(it, k):
            v = getattr(it, k)
            d[k] = jv(v) if not isinstance(v, str) else v
    return d


def tree(t):
    return {"name": t.name, "bl_idname": t.bl_idname, "users": t.users,
            "use_fake_user": t.use_fake_user, "is_embedded_data": t.is_embedded_data,
            "color_tag": t.color_tag, "description": t.description,
            "default_group_node_width": t.default_group_node_width,
            "items": [item(i) for i in t.interface.items_tree],
            "active_index": t.interface.active_index,
            "active": t.interface.active.name if t.interface.active else None,
            "nodes": [node(n) for n in t.nodes],
            "links": [[lk.from_node.name, lk.from_socket.identifier, lk.to_node.name,
                       lk.to_socket.identifier] for lk in t.links],
            "active_node": t.nodes.active.name if t.nodes.active else None}


def main():
    g = create_settings_group(get_gltf_node_name())
    out = {"group": tree(g)}
    mat = bpy.data.materials["Material"]
    gn = mat.node_tree.nodes.new("ShaderNodeGroup")
    out["fresh_group_node"] = node(gn)
    gn.node_tree = g
    out["group_node"] = node(gn)
    out["group_after"] = {"users": g.users}
    gn2 = mat.node_tree.nodes.new("ShaderNodeGroup")
    gn2.node_tree = g
    out["group_users_2"] = g.users
    gn2.node_tree = None
    out["group_node_cleared"] = node(gn2)
    out["group_users_after_clear"] = g.users
    out["second_name"] = create_settings_group(get_gltf_node_name()).name
    with open(sys.argv[sys.argv.index("--") + 1], "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1)


main()
