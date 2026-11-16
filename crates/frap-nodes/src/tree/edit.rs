// SPDX-License-Identifier: GPL-3.0-or-later
//! Socket lookup and graph editing: `inputs['Name']`, `nodes.remove`, `links.new`,
//! `links.remove` (ARCHITECTURE §16.1, WP 1.11).
//!
//! Verified on Blender 5.2.4:
//! - `inputs['A']` returns the **first enabled** socket named `A` (Mix `data_type='RGBA'` gives
//!   `A_Color`); identifiers are not keys (`inputs['A_Color']` is a `KeyError`).
//! - `links.new(a, b)` swaps its arguments when `a` is an input and `b` an output; two sockets
//!   of the same direction raise `RuntimeError: Error: Same input/output direction of sockets`.
//! - With `verify_limits=True` (the default) every link into the target input is removed first
//!   (input `link_limit` is 1) and the new link is appended last, even when the same link
//!   existed already. With `verify_limits=False` the link is just appended.
//! - `nodes.remove(n)` removes every link touching `n`; children of a removed frame lose
//!   their parent.
//! - A link from a SHADER output into a non-shader input, or from a node into itself, has
//!   `is_valid == False`.

use frap_rna::{PyExc, RnaError, RnaErrorKind, Value};

use crate::{Link, Node, NodeTree, Socket, SocketType};

/// `link_limit` of an input socket (shader and data inputs are single-input).
pub const INPUT_LINK_LIMIT: u32 = 1;
/// `link_limit` of an output socket.
pub const OUTPUT_LINK_LIMIT: u32 = 4095;

/// Index of the first enabled socket named `name` (Blender's name lookup rule).
pub fn find_socket(sockets: &[Socket], name: &str) -> Option<usize> {
    sockets.iter().position(|s| s.enabled && s.name == name)
}

/// One socket of a tree: node uid, direction and index in `inputs`/`outputs`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SocketAddr {
    /// Node uid.
    pub node: u32,
    /// True for `node.outputs`.
    pub is_output: bool,
    /// Index in `inputs` or `outputs`.
    pub index: usize,
}

impl SocketAddr {
    /// `node.inputs[index]`.
    pub fn input(node: u32, index: usize) -> Self {
        SocketAddr {
            node,
            is_output: false,
            index,
        }
    }
    /// `node.outputs[index]`.
    pub fn output(node: u32, index: usize) -> Self {
        SocketAddr {
            node,
            is_output: true,
            index,
        }
    }
}

impl Node {
    /// `node.inputs['name']`: the first enabled input with that name.
    pub fn input(&self, name: &str) -> Option<&Socket> {
        find_socket(&self.inputs, name).map(|i| &self.inputs[i])
    }
    /// Mutable `node.inputs['name']`.
    pub fn input_mut(&mut self, name: &str) -> Option<&mut Socket> {
        find_socket(&self.inputs, name).map(|i| &mut self.inputs[i])
    }
    /// Index of `node.inputs['name']`.
    pub fn input_index(&self, name: &str) -> Option<usize> {
        find_socket(&self.inputs, name)
    }
    /// `node.outputs['name']`: the first enabled output with that name.
    pub fn output(&self, name: &str) -> Option<&Socket> {
        find_socket(&self.outputs, name).map(|i| &self.outputs[i])
    }
    /// Index of `node.outputs['name']`.
    pub fn output_index(&self, name: &str) -> Option<usize> {
        find_socket(&self.outputs, name)
    }
    /// Input by identifier (links store identifiers).
    pub fn input_by_identifier(&self, identifier: &str) -> Option<usize> {
        self.inputs.iter().position(|s| s.identifier == identifier)
    }
    /// Output by identifier.
    pub fn output_by_identifier(&self, identifier: &str) -> Option<usize> {
        self.outputs.iter().position(|s| s.identifier == identifier)
    }
    /// The socket at `index` in `inputs` (`is_output == false`) or `outputs`.
    pub fn socket(&self, is_output: bool, index: usize) -> Option<&Socket> {
        if is_output {
            self.outputs.get(index)
        } else {
            self.inputs.get(index)
        }
    }
    /// A node property (`data_type`, `image`, …) by RNA identifier.
    pub fn prop(&self, identifier: &str) -> Option<&Value> {
        self.props.values.get(identifier)
    }
    /// An enum or string node property as `&str` (`""` when absent or of another kind).
    pub fn prop_str(&self, identifier: &str) -> &str {
        match self.prop(identifier) {
            Some(Value::Enum(s) | Value::Str(s)) => s.as_str(),
            _ => "",
        }
    }
    /// Writes a node property that the node type declares (value already coerced) and
    /// recomputes socket availability. Returns false when the node has no such property.
    pub fn set_prop(&mut self, identifier: &str, v: Value) -> bool {
        match self.props.values.get_mut(identifier) {
            Some(slot) => {
                *slot = v;
                self.update_availability();
                true
            }
            None => false,
        }
    }
}

fn same_direction() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "Error: Same input/output direction of sockets",
    )
}

fn bad_socket() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "Error: Unable to locate link sockets in node tree",
    )
}

impl NodeTree {
    /// Index of the node with this uid.
    pub fn node_index(&self, uid: u32) -> Option<usize> {
        self.nodes.iter().position(|n| n.uid == uid)
    }
    /// The node with this uid.
    pub fn node(&self, uid: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.uid == uid)
    }
    /// The mutable node with this uid.
    pub fn node_mut(&mut self, uid: u32) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.uid == uid)
    }
    /// `nodes['name']`.
    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }
    /// The first node of type `bl_idname`.
    pub fn first_of_type(&self, bl_idname: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.bl_idname == bl_idname)
    }

    /// The socket a link end refers to.
    pub fn socket_at(&self, a: SocketAddr) -> Option<&Socket> {
        self.node(a.node)?.socket(a.is_output, a.index)
    }

    /// The link into input `identifier` of node `uid`, if any (the first one).
    pub fn link_into(&self, uid: u32, identifier: &str) -> Option<&Link> {
        self.links
            .iter()
            .find(|l| l.to_node == uid && l.to_socket == identifier)
    }

    /// The socket feeding input `identifier` of node `uid`: (source node, output index).
    pub fn source_of(&self, uid: u32, identifier: &str) -> Option<(&Node, usize)> {
        let l = self.link_into(uid, identifier)?;
        let n = self.node(l.from_node)?;
        Some((n, n.output_by_identifier(&l.from_socket)?))
    }

    /// `socket.is_linked`.
    pub fn is_linked(&self, a: SocketAddr) -> bool {
        let Some(s) = self.socket_at(a) else {
            return false;
        };
        self.links.iter().any(|l| {
            if a.is_output {
                l.from_node == a.node && l.from_socket == s.identifier
            } else {
                l.to_node == a.node && l.to_socket == s.identifier
            }
        })
    }

    /// `link.is_valid`: false for a SHADER output into a non-shader input and for a link from
    /// a node into itself.
    pub fn link_is_valid(&self, l: &Link) -> bool {
        if l.from_node == l.to_node {
            return false;
        }
        let from = self
            .node(l.from_node)
            .and_then(|n| n.outputs.iter().find(|s| s.identifier == l.from_socket));
        let to = self
            .node(l.to_node)
            .and_then(|n| n.inputs.iter().find(|s| s.identifier == l.to_socket));
        match (from, to) {
            (Some(f), Some(t)) => !(f.ty == SocketType::Shader && t.ty != SocketType::Shader),
            _ => false,
        }
    }

    /// `node_tree.links.new(a, b, verify_limits)`: returns the index of the new link in
    /// `links` (always the last one).
    pub fn link_new(
        &mut self,
        a: SocketAddr,
        b: SocketAddr,
        verify_limits: bool,
    ) -> Result<usize, RnaError> {
        if a.is_output == b.is_output {
            return Err(same_direction());
        }
        let (from, to) = if a.is_output { (a, b) } else { (b, a) };
        let from_id = self
            .socket_at(from)
            .ok_or_else(bad_socket)?
            .identifier
            .clone();
        let to_id = self
            .socket_at(to)
            .ok_or_else(bad_socket)?
            .identifier
            .clone();
        if verify_limits {
            // Input link_limit is 1: replace whatever feeds the input. Outputs (4095) never
            // reach their limit in practice.
            self.links
                .retain(|l| !(l.to_node == to.node && l.to_socket == to_id));
            let from_count = self
                .links
                .iter()
                .filter(|l| l.from_node == from.node && l.from_socket == from_id)
                .count();
            if from_count >= OUTPUT_LINK_LIMIT as usize {
                self.links
                    .retain(|l| !(l.from_node == from.node && l.from_socket == from_id));
            }
        }
        self.links.push(Link {
            from_node: from.node,
            from_socket: from_id,
            to_node: to.node,
            to_socket: to_id,
        });
        Ok(self.links.len() - 1)
    }

    /// `links.new` by socket names (first enabled), for Rust callers (`raise`, tests).
    pub fn link_by_name(&mut self, from: (u32, &str), to: (u32, &str)) -> Result<usize, RnaError> {
        let f = self
            .node(from.0)
            .and_then(|n| n.output_index(from.1))
            .ok_or_else(bad_socket)?;
        let t = self
            .node(to.0)
            .and_then(|n| n.input_index(to.1))
            .ok_or_else(bad_socket)?;
        self.link_new(
            SocketAddr::output(from.0, f),
            SocketAddr::input(to.0, t),
            true,
        )
    }

    /// `node_tree.links.remove(link)` by index.
    pub fn remove_link(&mut self, index: usize) -> Option<Link> {
        (index < self.links.len()).then(|| self.links.remove(index))
    }

    /// `node_tree.nodes.remove(node)` by index: drops the links touching it and un-parents its
    /// children.
    pub fn remove_node(&mut self, index: usize) -> Option<Node> {
        if index >= self.nodes.len() {
            return None;
        }
        let n = self.nodes.remove(index);
        self.links
            .retain(|l| l.from_node != n.uid && l.to_node != n.uid);
        for c in &mut self.nodes {
            if c.parent == Some(n.uid) {
                c.parent = None;
            }
        }
        Some(n)
    }

    /// `node.location_absolute`: the location plus every parent frame's location.
    pub fn location_absolute(&self, index: usize) -> [f32; 2] {
        let mut loc = self.nodes[index].location;
        let mut parent = self.nodes[index].parent;
        let mut guard = 0;
        while let Some(p) = parent.and_then(|u| self.node(u)) {
            loc[0] += p.location[0];
            loc[1] += p.location[1];
            parent = p.parent;
            guard += 1;
            if guard > self.nodes.len() {
                break;
            }
        }
        loc
    }
}
