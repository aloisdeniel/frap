// SPDX-License-Identifier: GPL-3.0-or-later
//! Group sockets (WP 3.1): the interface of a node group (`tree.interface`) declares the
//! outputs of its `Group Input` nodes, the inputs of its `Group Output` nodes and the sockets of
//! every group node (`ShaderNodeGroup`) whose `node_tree` is the group. These are the
//! availability rules of the group node types, which depend on another tree instead of a mode
//! property, so they are run by the callers that change an interface or a `node_tree`.
//!
//! Verified on Blender 5.2.4:
//! - `interface.new_socket(name, in_out='INPUT', socket_type='NodeSocketFloat')` names the item
//!   `Socket_<n>` from a counter of the tree (never reused; frap continues after the highest
//!   number present, the same until a removal of the last item); output items are kept before
//!   the input items (`position`/`index` follow `items_tree` order); socket types are the
//!   `NodeSocketFloat|Int|Bool|Vector|Color|Menu|Shader|Bundle|Closure` classes and 'DEFAULT'
//!   (a float). Fresh values: 0, 0, False, (0, 0, 0), (0, 0, 0, 1), '' (menu).
//! - `Group Input` has one output per input item (identifier, name and type of the item, its
//!   value taken from the item **when the socket is created**: a later write of the item's
//!   `default_value` changes no existing socket) and a last virtual `__extend__` output;
//!   `Group Output` has one input per output item and a last `__extend__` input. A group node
//!   has the item sockets only (no `__extend__`); a group node without `node_tree` has none.
//! - Renaming an item renames the sockets; removing one removes its sockets and their links;
//!   values of existing sockets are kept across interface changes.

// Ported from blender/source/blender/nodes/intern/node_common.cc (`node_group_declare`,
// `node_group_input_declare`, `node_group_output_declare`) and
// blender/source/blender/blenkernel/intern/node_tree_interface.cc (`add_socket`,
// `find_valid_insert_position_for_item`), Blender 5.2.4, GPL-2.0-or-later.

use frap_rna::{IdAny, Value};
use smol_str::{SmolStr, format_smolstr};

use crate::generated::node_type;
use crate::{InOut, InterfaceSocket, Node, NodeTree, Socket, SocketType, SocketValue};

/// `bl_idname` of the group input node.
pub const GROUP_INPUT: &str = "NodeGroupInput";
/// `bl_idname` of the group output node.
pub const GROUP_OUTPUT: &str = "NodeGroupOutput";
/// Identifier (and empty name) of the virtual socket that extends a group interface.
pub const EXTEND_SOCKET: &str = "__extend__";

/// Whether nodes of this type instance a group through their `node_tree` property.
pub fn is_group_node(bl_idname: &str) -> bool {
    matches!(bl_idname, "ShaderNodeGroup" | "GeometryNodeGroup")
}

/// The socket type of an interface `socket_type` (`NodeSocketFloat`, …; `DEFAULT` is a float).
/// `None` for the classes frap cannot store (`NodeSocketBundle`, `NodeSocketClosure`).
pub fn interface_socket_type(socket_type: &str) -> Option<SocketType> {
    Some(match socket_type {
        "DEFAULT" | "NodeSocketFloat" => SocketType::Value,
        "NodeSocketInt" => SocketType::Int,
        "NodeSocketBool" => SocketType::Boolean,
        "NodeSocketVector" => SocketType::Vector,
        "NodeSocketColor" => SocketType::Rgba,
        "NodeSocketMenu" => SocketType::Menu,
        "NodeSocketShader" => SocketType::Shader,
        _ => return None,
    })
}

/// The `socket_type` / `bl_socket_idname` of an interface item of type `ty`.
pub fn interface_socket_idname(ty: SocketType) -> &'static str {
    match ty {
        SocketType::Value => "NodeSocketFloat",
        SocketType::Int => "NodeSocketInt",
        SocketType::Boolean => "NodeSocketBool",
        SocketType::Vector => "NodeSocketVector",
        SocketType::Rgba => "NodeSocketColor",
        SocketType::Menu => "NodeSocketMenu",
        SocketType::Shader => "NodeSocketShader",
        SocketType::String => "NodeSocketString",
        SocketType::Rotation => "NodeSocketRotation",
        SocketType::Matrix => "NodeSocketMatrix",
        SocketType::Geometry => "NodeSocketGeometry",
        SocketType::Object => "NodeSocketObject",
        SocketType::Image => "NodeSocketImage",
        SocketType::Collection => "NodeSocketCollection",
        SocketType::Material => "NodeSocketMaterial",
        SocketType::Texture => "NodeSocketTexture",
        _ => "NodeSocketVirtual",
    }
}

/// The value of a fresh interface item of type `ty` (verified per class on 5.2.4).
pub fn interface_default(ty: SocketType) -> SocketValue {
    match ty {
        SocketType::Value => SocketValue::Float(0.0),
        SocketType::Int => SocketValue::Int(0),
        SocketType::Boolean => SocketValue::Bool(false),
        SocketType::Vector => SocketValue::Vector([0.0; 4]),
        SocketType::Rgba => SocketValue::Color([0.0, 0.0, 0.0, 1.0]),
        SocketType::Menu => SocketValue::Menu(SmolStr::default()),
        SocketType::String => SocketValue::String(SmolStr::default()),
        SocketType::Rotation => SocketValue::Rotation([0.0; 3]),
        SocketType::Object
        | SocketType::Image
        | SocketType::Collection
        | SocketType::Material
        | SocketType::Texture => SocketValue::Id(None),
        _ => SocketValue::None,
    }
}

fn extend_socket() -> Socket {
    Socket {
        name: SmolStr::default(),
        identifier: SmolStr::new_static(EXTEND_SOCKET),
        ty: SocketType::Custom,
        enabled: true,
        hide_value: false,
        default_value: SocketValue::None,
    }
}

/// The sockets declared by `items`, keeping the value of an existing socket with the same
/// identifier and type (`old`), else taking the item's value.
fn declare<'a>(
    items: impl Iterator<Item = &'a InterfaceSocket>,
    old: &[Socket],
    extend: bool,
) -> Vec<Socket> {
    let mut out: Vec<Socket> = items
        .map(|it| {
            let kept = old
                .iter()
                .find(|s| s.identifier == it.identifier && s.ty == it.socket_type);
            Socket {
                name: it.name.clone(),
                identifier: it.identifier.clone(),
                ty: it.socket_type,
                enabled: true,
                hide_value: kept.is_some_and(|s| s.hide_value),
                default_value: kept
                    .map_or_else(|| it.default_value.clone(), |s| s.default_value.clone()),
            }
        })
        .collect();
    if extend {
        out.push(extend_socket());
    }
    out
}

impl Node {
    /// Rebuilds the sockets of a group node from the interface of `group` (`None`: the
    /// `node_tree` is unset, no sockets). Links are not touched: use
    /// [`NodeTree::sync_group_node`] on a node of a tree.
    pub fn sync_group_sockets(&mut self, group: Option<&[InterfaceSocket]>) {
        let items = group.unwrap_or_default();
        self.inputs = declare(
            items.iter().filter(|s| s.in_out == InOut::Input),
            &self.inputs,
            false,
        );
        self.outputs = declare(
            items.iter().filter(|s| s.in_out == InOut::Output),
            &self.outputs,
            false,
        );
    }

    /// The group this group node instances (`node.node_tree`), if any.
    pub fn group_tree(&self) -> Option<IdAny> {
        if !is_group_node(self.bl_idname) {
            return None;
        }
        match self.prop("node_tree") {
            Some(Value::Id(id)) => *id,
            _ => None,
        }
    }
}

impl NodeTree {
    /// The number `n` of the next `Socket_<n>` item identifier.
    fn next_interface_uid(&self) -> u32 {
        self.interface
            .iter()
            .filter_map(|s| s.identifier.strip_prefix("Socket_")?.parse::<u32>().ok())
            .map(|n| n + 1)
            .max()
            .unwrap_or(0)
    }

    /// `interface.new_socket(name, in_out=…, socket_type=…)`: adds an item of type `ty`
    /// (outputs before inputs), refreshes this tree's group input/output nodes and returns the
    /// item's index in `interface`. Group nodes of other trees are refreshed by the caller
    /// ([`NodeTree::sync_group_nodes_of`]).
    pub fn interface_new_socket(&mut self, name: &str, in_out: InOut, ty: SocketType) -> usize {
        let item = InterfaceSocket {
            name: SmolStr::new(name),
            identifier: format_smolstr!("Socket_{}", self.next_interface_uid()),
            in_out,
            socket_type: ty,
            default_value: interface_default(ty),
        };
        let at = match in_out {
            InOut::Output => self
                .interface
                .iter()
                .rposition(|s| s.in_out == InOut::Output)
                .map_or(0, |p| p + 1),
            InOut::Input => self.interface.len(),
        };
        self.interface.insert(at, item);
        self.sync_group_io_nodes();
        at
    }

    /// `interface.remove(item)`: removes item `index` (no-op when out of range) and refreshes
    /// this tree's group input/output nodes (their sockets and links).
    pub fn interface_remove(&mut self, index: usize) {
        if index < self.interface.len() {
            self.interface.remove(index);
            self.sync_group_io_nodes();
        }
    }

    /// `interface.clear()`.
    pub fn interface_clear(&mut self) {
        self.interface.clear();
        self.sync_group_io_nodes();
    }

    /// Renames item `index` (`item.name = …`) and its sockets on this tree's group
    /// input/output nodes.
    pub fn interface_rename(&mut self, index: usize, name: &str) {
        if let Some(it) = self.interface.get_mut(index) {
            it.name = SmolStr::new(name);
            self.sync_group_io_nodes();
        }
    }

    /// Re-declares the sockets of every `Group Input` / `Group Output` node of this tree from
    /// its interface and removes the links of sockets that no longer exist.
    pub fn sync_group_io_nodes(&mut self) {
        let iface = &self.interface;
        for n in &mut self.nodes {
            match n.bl_idname {
                GROUP_INPUT => {
                    n.outputs = declare(
                        iface.iter().filter(|s| s.in_out == InOut::Input),
                        &n.outputs,
                        true,
                    );
                }
                GROUP_OUTPUT => {
                    n.inputs = declare(
                        iface.iter().filter(|s| s.in_out == InOut::Output),
                        &n.inputs,
                        true,
                    );
                }
                _ => {}
            }
        }
        self.prune_dangling_links();
    }

    /// Re-declares the sockets of node `index` (a group node) from `group`'s interface and
    /// removes the links of sockets that no longer exist.
    pub fn sync_group_node(&mut self, index: usize, group: Option<&[InterfaceSocket]>) {
        if let Some(n) = self.nodes.get_mut(index) {
            n.sync_group_sockets(group);
            self.prune_dangling_links();
        }
    }

    /// Re-declares every group node of this tree that instances `group` (`node_tree` is
    /// `group_id`); returns whether there was one.
    pub fn sync_group_nodes_of(&mut self, group_id: IdAny, group: &[InterfaceSocket]) -> bool {
        let mut any = false;
        for n in &mut self.nodes {
            if n.group_tree() == Some(group_id) {
                n.sync_group_sockets(Some(group));
                any = true;
            }
        }
        if any {
            self.prune_dangling_links();
        }
        any
    }

    /// Adds a group node (`nodes.new('ShaderNodeGroup')`, named "Group" made unique) that
    /// instances `group` (`node.node_tree = group`) and returns its index.
    pub fn add_group_node(&mut self, group_id: IdAny, group: &[InterfaceSocket]) -> usize {
        let def = node_type(match self.ty {
            crate::NodeTreeType::Geometry => "GeometryNodeGroup",
            _ => "ShaderNodeGroup",
        })
        .or_else(|| node_type("ShaderNodeGroup"))
        .expect("invariant: generated group node type");
        let i = self.add_node(def);
        let n = &mut self.nodes[i];
        n.props
            .values
            .insert("node_tree", Value::Id(Some(group_id)));
        n.sync_group_sockets(Some(group));
        i
    }

    /// Removes links whose end socket no longer exists (Blender frees them with the socket).
    fn prune_dangling_links(&mut self) {
        let nodes = &self.nodes;
        let exists = |uid: u32, id: &str, output: bool| {
            nodes.iter().find(|n| n.uid == uid).is_some_and(|n| {
                let s = if output { &n.outputs } else { &n.inputs };
                s.iter().any(|s| s.identifier == id)
            })
        };
        self.links.retain(|l| {
            exists(l.from_node, &l.from_socket, true) && exists(l.to_node, &l.to_socket, false)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeTreeType;

    #[test]
    fn new_socket_identifiers_order_and_io_nodes() {
        let mut g = NodeTree::new("G", NodeTreeType::Shader, false);
        assert_eq!(
            g.interface_new_socket("a", InOut::Input, SocketType::Value),
            0
        );
        assert_eq!(
            g.interface_new_socket("o", InOut::Output, SocketType::Shader),
            0
        );
        assert_eq!(
            g.interface_new_socket("b", InOut::Input, SocketType::Rgba),
            2
        );
        let ids: Vec<_> = g.interface.iter().map(|s| s.identifier.as_str()).collect();
        assert_eq!(ids, ["Socket_1", "Socket_0", "Socket_2"]);
        let gi = g.new_node(GROUP_INPUT).unwrap();
        let go = g.new_node(GROUP_OUTPUT).unwrap();
        g.sync_group_io_nodes();
        let outs: Vec<_> = g.nodes[gi]
            .outputs
            .iter()
            .map(|s| s.identifier.as_str())
            .collect();
        assert_eq!(outs, ["Socket_0", "Socket_2", "__extend__"]);
        let ins: Vec<_> = g.nodes[go]
            .inputs
            .iter()
            .map(|s| s.identifier.as_str())
            .collect();
        assert_eq!(ins, ["Socket_1", "__extend__"]);
        assert_eq!(
            g.nodes[gi].outputs[1].default_value,
            SocketValue::Color([0.0, 0.0, 0.0, 1.0])
        );
    }

    #[test]
    fn values_are_kept_and_links_pruned() {
        let mut g = NodeTree::new("G", NodeTreeType::Shader, false);
        g.interface_new_socket("a", InOut::Input, SocketType::Value);
        let mut m = NodeTree::default_material();
        let gid = frap_rna::IdRef::<frap_rna::NodeTreeId>::from_key(frap_rna::IdKey::next()).any();
        let n = m.add_group_node(gid, &g.interface);
        assert_eq!(m.nodes[n].name, "Group");
        m.nodes[n].inputs[0].default_value = SocketValue::Float(3.0);
        g.interface[0].default_value = SocketValue::Float(9.0);
        g.interface_new_socket("o", InOut::Output, SocketType::Shader);
        assert!(m.sync_group_nodes_of(gid, &g.interface));
        assert_eq!(m.nodes[n].inputs[0].default_value, SocketValue::Float(3.0));
        let out = m.node_index(m.nodes[1].uid).unwrap();
        let (from, to) = (m.nodes[n].uid, m.nodes[out].uid);
        m.links.push(crate::Link {
            from_node: from,
            from_socket: "Socket_1".into(),
            to_node: to,
            to_socket: "Surface".into(),
        });
        g.interface_remove(0);
        m.sync_group_nodes_of(gid, &g.interface);
        assert_eq!(m.nodes[n].outputs.len(), 0);
        assert_eq!(
            m.links.len(),
            1,
            "the group's link is gone, the default one kept"
        );
    }
}
