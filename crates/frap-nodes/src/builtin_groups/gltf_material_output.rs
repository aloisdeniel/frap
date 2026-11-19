// SPDX-License-Identifier: GPL-3.0-or-later
//! The "glTF Material Output" node group of the glTF add-on (ARCHITECTURE §16.1, WP 3.1): a
//! **local** `bpy.data.node_groups` shader group that tells the exporter about inputs the
//! Principled BSDF has no socket for (core `occlusionTexture`, `KHR_materials_volume`
//! thickness, `KHR_materials_dispersion`, `KHR_materials_iridescence`). It has no effect on
//! shading. The importer (`make_settings_node`) and the add-on's "glTF Material Output" menu
//! entry reuse an existing group of that exact name and create it otherwise.
//!
//! Verified on 5.2.4 by running the add-on's `create_settings_group(get_gltf_node_name())`
//! (`blender/com/material_helpers.py`), dump in `_gltf_material_output.blender.json`:
//! - five float inputs `Occlusion` (`Socket_0`, 0), `Thickness` (`Socket_1`, 0), `Dispersion`
//!   (`Socket_2`, 0), `Iridescence Factor` (`Socket_3`, 0) and `Iridescence Thickness Minimum`
//!   (`Socket_4`, 100), no output, no panel (the add-on of 5.2.4 has no "Iridescence Thickness
//!   Maximum" input, although ARCHITECTURE §16.1 listed one);
//! - nodes `Group Output` at (0, 0), then `Group Input` at (−200, 0), no link;
//! - the group has no user until a group node uses it (`users` 1 per group node); a second
//!   creation is named `glTF Material Output.001`.
//! - a group node using it (`nodes.new('ShaderNodeGroup')`, named "Group", then
//!   `node.node_tree = group`) gets the five inputs with the item values, no output.

// Ported from blender/scripts/addons_core/io_scene_gltf2/blender/com/material_helpers.py
// (`create_settings_group`, `get_gltf_node_name`, `get_gltf_node_old_name`), Blender 5.2.4,
// Apache-2.0 (the glTF-Blender-IO add-on).

use crate::{GROUP_INPUT, GROUP_OUTPUT, InOut, NodeTree, NodeTreeType, SocketType, SocketValue};

/// `get_gltf_node_name()`: the group's name, which the exporter looks for.
pub const NAME: &str = "glTF Material Output";
/// `get_gltf_node_old_name()`: the name of the group in files of older add-ons, still
/// recognised by the exporter.
pub const OLD_NAME: &str = "glTF Settings";

/// Interface inputs in `create_settings_group` order: (name, default value).
pub const INPUTS: [(&str, f32); 5] = [
    ("Occlusion", 0.0),
    ("Thickness", 0.0),
    ("Dispersion", 0.0),
    ("Iridescence Factor", 0.0),
    ("Iridescence Thickness Minimum", 100.0),
];

/// The name test of the exporter's `get_socket_from_gltf_material_node`: the group name starts
/// with [`NAME`] or [`OLD_NAME`], ignoring case (so `glTF Material Output.001` matches).
///
/// Caution for exporters (verified in the 5.2.4 add-on, `exp/material/search_node_tree.py`):
/// the add-on writes this test as `any([[… for g in names]])`, a list in a list, which is
/// always true, so it actually reads the first input with the wanted name (`Occlusion`,
/// `Thickness`, …) of **any** group node that has a `node_tree`, whatever the group's name.
/// The importer and `create_settings_group` look the group up by the exact name [`NAME`].
pub fn is_gltf_material_output(group_name: &str) -> bool {
    let lower = group_name.to_lowercase();
    [NAME, OLD_NAME]
        .iter()
        .any(|n| lower.starts_with(&n.to_lowercase()))
}

/// `create_settings_group(NAME)`: the group as the add-on creates it (`bpy.data.node_groups.new`
/// is the caller's: it inserts the tree into `node_groups`, which may suffix the name).
pub fn new_group() -> NodeTree {
    let mut g = NodeTree::new(NAME, NodeTreeType::Shader, false);
    for (name, default) in INPUTS {
        let i = g.interface_new_socket(name, InOut::Input, SocketType::Value);
        g.interface[i].default_value = SocketValue::Float(default);
    }
    g.new_node(GROUP_OUTPUT)
        .expect("invariant: generated node type");
    let gi = g
        .new_node(GROUP_INPUT)
        .expect("invariant: generated node type");
    g.nodes[gi].location = [-200.0, 0.0];
    g.sync_group_io_nodes();
    g
}

#[cfg(test)]
mod tests {
    //! The group equals the add-on's (Blender dump of `create_settings_group` and of a group
    //! node using it, `_gltf_material_output.blender.json`).

    use serde_json::Value as J;

    use super::*;
    use crate::{EXTEND_SOCKET, Socket, rna};

    const DUMP: &str = include_str!("_gltf_material_output.blender.json");

    fn f(v: &J) -> f32 {
        v.as_f64().expect("number") as f32
    }

    fn check_socket(s: &Socket, j: &J, ctx: &str) {
        assert_eq!(s.name, j["name"].as_str().unwrap(), "{ctx}");
        assert_eq!(s.identifier, j["identifier"].as_str().unwrap(), "{ctx}");
        assert_eq!(
            rna::socket_type_str(s.ty),
            j["type"].as_str().unwrap(),
            "{ctx}"
        );
        assert_eq!(s.enabled, j["enabled"].as_bool().unwrap(), "{ctx}");
        assert_eq!(s.hide_value, j["hide_value"].as_bool().unwrap(), "{ctx}");
        match (&s.default_value, j.get("default_value")) {
            (SocketValue::Float(x), Some(v)) => assert_eq!(*x, f(v), "{ctx}"),
            (SocketValue::None, None) => {}
            (other, v) => panic!("{ctx}: {other:?} vs {v:?}"),
        }
    }

    #[test]
    fn group_equals_the_add_on_group() {
        let dump: J = serde_json::from_str(DUMP).unwrap();
        let jg = &dump["group"];
        let g = new_group();
        assert_eq!(g.header.name, jg["name"].as_str().unwrap());
        assert!(!g.header.is_embedded_data);
        assert_eq!(g.ty, NodeTreeType::Shader);
        let items = jg["items"].as_array().unwrap();
        assert_eq!(g.interface.len(), items.len());
        for (it, j) in g.interface.iter().zip(items) {
            assert_eq!(it.name, j["name"].as_str().unwrap());
            assert_eq!(it.identifier, j["identifier"].as_str().unwrap());
            assert_eq!(it.in_out, InOut::Input);
            assert_eq!(j["in_out"].as_str(), Some("INPUT"));
            assert_eq!(
                crate::interface_socket_idname(it.socket_type),
                j["socket_type"].as_str().unwrap()
            );
            assert_eq!(it.default_value, SocketValue::Float(f(&j["default_value"])));
        }
        let nodes = jg["nodes"].as_array().unwrap();
        assert_eq!(g.nodes.len(), nodes.len());
        for (n, j) in g.nodes.iter().zip(nodes) {
            let ctx = n.name.as_str();
            assert_eq!(n.name, j["name"].as_str().unwrap());
            assert_eq!(n.bl_idname, j["bl_idname"].as_str().unwrap());
            assert_eq!(n.location[0], f(&j["location"][0]), "{ctx}");
            assert_eq!(n.location[1], f(&j["location"][1]), "{ctx}");
            assert_eq!(n.width, f(&j["width"]), "{ctx}");
            for (dir, sockets) in [("inputs", &n.inputs), ("outputs", &n.outputs)] {
                let js = j[dir].as_array().unwrap();
                assert_eq!(sockets.len(), js.len(), "{ctx} {dir}");
                for (s, js) in sockets.iter().zip(js) {
                    check_socket(s, js, &format!("{ctx} {dir} {}", s.identifier));
                }
            }
        }
        assert!(g.links.is_empty() && jg["links"].as_array().unwrap().is_empty());
        assert_eq!(g.nodes[1].outputs.last().unwrap().identifier, EXTEND_SOCKET);
    }

    /// Every property of the dumped interface items reads the same through frap's RNA
    /// (`NodeTreeInterfaceSocketFloat`), and `interface.active` is the first item.
    #[test]
    fn interface_rna_equals_blender() {
        use frap_rna::{Field, Rna, Value};

        let dump: J = serde_json::from_str(DUMP).unwrap();
        let g = new_group();
        let iface = rna::interface::InterfaceRef { tree: &g };
        let srna = iface.srna();
        let active = srna.find_prop("active").unwrap();
        match iface.get(active) {
            Ok(Field::Struct(s)) => {
                let name = s.srna().find_prop("name").unwrap();
                assert!(matches!(s.get(name), Ok(Field::Value(Value::Str(n))) if n == "Occlusion"));
            }
            _ => panic!("interface.active"),
        }
        let items = dump["group"]["items"].as_array().unwrap();
        for (i, j) in items.iter().enumerate() {
            let item = rna::interface::ItemRef { tree: &g, i };
            assert_eq!(item.srna().identifier, "NodeTreeInterfaceSocketFloat");
            for (prop, want) in j.as_object().unwrap() {
                let p = item
                    .srna()
                    .find_prop(prop)
                    .unwrap_or_else(|| panic!("{prop} is not an RNA property"));
                let got = match item.get(p) {
                    Ok(Field::Value(Value::Str(s) | Value::Enum(s))) => J::from(s.as_str()),
                    Ok(Field::Value(Value::Bool(b))) => J::from(b),
                    Ok(Field::Value(Value::Int(x))) => J::from(x),
                    Ok(Field::Value(Value::Float(x))) => J::from(f64::from(x)),
                    other => panic!("{prop}: {:?}", other.err()),
                };
                match (got.as_f64(), want.as_f64()) {
                    (Some(a), Some(b)) => {
                        assert!((a - b).abs() <= 1e-6 * b.abs().max(1.0), "{prop}")
                    }
                    _ => assert_eq!(&got, want, "item {i}.{prop}"),
                }
            }
        }
    }

    #[test]
    fn group_node_equals_blender() {
        let dump: J = serde_json::from_str(DUMP).unwrap();
        let g = new_group();
        let mut m = NodeTree::default_material();
        let gid = frap_rna::IdRef::<frap_rna::NodeTreeId>::from_key(frap_rna::IdKey::next()).any();
        let fresh = m.new_node("ShaderNodeGroup").unwrap();
        assert!(m.nodes[fresh].inputs.is_empty() && m.nodes[fresh].outputs.is_empty());
        let i = m.add_group_node(gid, &g.interface);
        let j = &dump["group_node"];
        let n = &m.nodes[i];
        assert_eq!(n.name, "Group.001", "the fresh node took \"Group\"");
        assert_eq!(n.width, f(&j["width"]));
        let js = j["inputs"].as_array().unwrap();
        assert_eq!(n.inputs.len(), js.len());
        for (s, js) in n.inputs.iter().zip(js) {
            check_socket(s, js, &format!("group node {}", s.identifier));
        }
        assert!(n.outputs.is_empty() && j["outputs"].as_array().unwrap().is_empty());
        assert_eq!(n.group_tree(), Some(gid));
        assert!(is_gltf_material_output("glTF Material Output"));
        assert!(is_gltf_material_output("glTF Settings"));
        assert!(is_gltf_material_output("gltf material output.001"));
        assert!(!is_gltf_material_output("My glTF Material Output"));
    }
}
