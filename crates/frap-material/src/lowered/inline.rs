// SPDX-License-Identifier: GPL-3.0-or-later
//! The node-group inlining and the muted-node bypass of `InlineShaderNodes` (WP 8.12): the
//! tree the glTF exporter reads has no group node and no link through a muted node, so a
//! texture inside a node group, or behind a muted node, is exported.
//!
//! Ported from blender/source/blender/nodes/intern/shader_nodes_inline.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `handle_output_socket__group` (a group node's output is the matching
//! input of the group's output node, `group_output_node()`: the first Group Output),
//! `handle_output_socket__group_input` (a Group Input's output is the matching input of the
//! group node, in the parent context), `handle_input_socket` (an unlinked input holds its own
//! value; an unlinked input of a reroute or a muted node is *dangling*, and a consumer of a
//! dangling value keeps its own value), `handle_output_socket__internal_links` (a muted node
//! passes the input of its internal link, `find_internally_linked_input`; without one its
//! output is dangling) and `handle_implicit_conversion` (a constant reaching an input of
//! another type is converted, `FoldVal::to`).
//!
//! frap keeps the rest of the material tree as it is (the patterns emulate the inliner's
//! constant folding on top, `graph/fold.rs`): the expansion splices every non-muted group
//! node whose group has a Group Output into the tree (nested groups first), then rewires the
//! links leaving muted nodes. Group nodes without a group tree or Group Output (Blender's
//! fallback value) and muted group nodes stay; so do the muted nodes themselves (only their
//! outgoing links change).

use frap_nodes::{Link, Node, NodeTree, SocketType, SocketValue};
use frap_rna::IdAny;
use smol_str::SmolStr;

use crate::graph::FoldVal;
use crate::{ImageLookup, LowerCx};

/// The deepest group nesting expanded (Blender refuses recursive groups; frap guards).
const MAX_DEPTH: usize = 32;

/// Where a value comes from while splicing.
#[derive(Clone, Debug)]
enum Source {
    /// A link from this node's output socket.
    Link(u32, SmolStr),
    /// A constant of this type.
    Const(SocketType, SocketValue),
}

/// The group tree of a non-muted group node that can be inlined.
fn group_of<'g>(n: &Node, images: &'g dyn ImageLookup) -> Option<&'g NodeTree> {
    if !frap_nodes::is_group_node(n.bl_idname) || n.mute {
        return None;
    }
    let id: IdAny = n.group_tree()?;
    let g = images.node_group(id)?;
    g.nodes
        .iter()
        .any(|x| x.bl_idname == "NodeGroupOutput")
        .then_some(g)
}

/// `value` of type `from` as a default value of a socket of type `to` (the implicit
/// conversion), `None` when frap does not convert between the two.
fn convert(from: SocketType, value: &SocketValue, to: SocketType) -> Option<SocketValue> {
    if from == to {
        return Some(value.clone());
    }
    let f = FoldVal::of_socket(from, value)?.to(to)?;
    Some(match f {
        FoldVal::Float(x) => SocketValue::Float(x),
        FoldVal::Vector(v) => SocketValue::Vector([v[0], v[1], v[2], 0.0]),
        FoldVal::Color(c) => SocketValue::Color(c),
    })
}

/// The type of input `identifier` of node `uid`.
fn input_type(tree: &NodeTree, uid: u32, identifier: &str) -> Option<SocketType> {
    tree.node(uid)?
        .inputs
        .iter()
        .find(|s| s.identifier == identifier)
        .map(|s| s.ty)
}

/// Connects `src` to input `(uid, identifier)` of `tree`: a link, or the converted constant as
/// the input's own value (left as is when the types do not convert).
fn connect(
    tree: &mut NodeTree,
    src: &Source,
    uid: u32,
    identifier: &SmolStr,
    links: &mut Vec<Link>,
) {
    match src {
        Source::Link(from, socket) => links.push(Link {
            from_node: *from,
            from_socket: socket.clone(),
            to_node: uid,
            to_socket: identifier.clone(),
        }),
        Source::Const(ty, v) => {
            let Some(to_ty) = input_type(tree, uid, identifier) else {
                return;
            };
            if let Some(v) = convert(*ty, v, to_ty)
                && let Some(s) = tree
                    .node_mut(uid)
                    .and_then(|n| n.inputs.iter_mut().find(|s| s.identifier == *identifier))
            {
                s.default_value = v;
            }
        }
    }
}

/// How a group is spliced.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mode {
    /// `InlineShaderNodes`: inner nodes renamed `group/node`, an unlinked Group Output input
    /// is a constant for the group's consumers.
    Inline,
    /// The add-on's traversal of the original tree (`NodeNav.descend`/`ascend`,
    /// `from_socket`'s GROUP and GROUP_INPUT cases): inner nodes keep their names (the
    /// add-on's `nodes_used` holds `node.name`), the group node stays in the tree, and an
    /// unlinked Group Output input is a dead end, no constant (`get_constant` stops on the
    /// Group Output node).
    Original,
}

/// Every group of `tree` inlined, recursively (`depth` levels left).
fn flatten(tree: &NodeTree, images: &dyn ImageLookup, depth: usize, mode: Mode) -> NodeTree {
    let mut t = tree.clone();
    if depth == 0 {
        return t;
    }
    // The group nodes of this level (the inner trees are flattened first, so the nodes a
    // splice adds need no second pass; in `Original` mode the group nodes stay in the tree).
    let groups: Vec<(u32, &NodeTree)> = tree
        .nodes
        .iter()
        .filter_map(|n| group_of(n, images).map(|g| (n.uid, g)))
        .collect();
    for (uid, inner) in groups {
        let inner = flatten(inner, images, depth - 1, mode);
        splice(&mut t, uid, &inner, mode);
    }
    t
}

/// Replaces group node `g` of `t` by the nodes of its (flattened) group tree `inner`.
fn splice(t: &mut NodeTree, g: u32, inner: &NodeTree, mode: Mode) {
    let Some(group) = t.node(g).cloned() else {
        return;
    };
    let mut next = t.nodes.iter().map(|n| n.uid).max().unwrap_or(0) + 1;
    let output = inner
        .nodes
        .iter()
        .find(|n| n.bl_idname == "NodeGroupOutput")
        .map(|n| n.uid);
    let is_io = |n: &Node| matches!(n.bl_idname, "NodeGroupInput" | "NodeGroupOutput");
    // New uids for the kept inner nodes.
    let mut map: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut added = Vec::new();
    for n in inner.nodes.iter().filter(|n| !is_io(n)) {
        let mut c = n.clone();
        c.uid = next;
        if mode == Mode::Inline {
            c.name = SmolStr::new(format!("{}/{}", group.name, n.name));
        }
        c.parent = None;
        map.insert(n.uid, next);
        next += 1;
        added.push(c);
    }
    let input_node = |uid: u32| {
        inner
            .node(uid)
            .is_some_and(|n| n.bl_idname == "NodeGroupInput")
    };
    // The value of the group node's input `identifier` in the parent tree.
    let group_input = |identifier: &str| -> Option<Source> {
        let s = group.inputs.iter().find(|s| s.identifier == identifier)?;
        Some(match t.link_into(g, identifier) {
            Some(l) => Source::Link(l.from_node, l.from_socket.clone()),
            None => Source::Const(s.ty, s.default_value.clone()),
        })
    };
    // The value of the group's output `identifier` (the Group Output's input).
    let group_output = |identifier: &str| -> Option<Source> {
        let out = output?;
        let socket = inner
            .node(out)?
            .inputs
            .iter()
            .find(|s| s.identifier == identifier)?;
        Some(match inner.link_into(out, identifier) {
            Some(l) if input_node(l.from_node) => group_input(&l.from_socket)?,
            Some(l) => Source::Link(*map.get(&l.from_node)?, l.from_socket.clone()),
            None => Source::Const(socket.ty, socket.default_value.clone()),
        })
    };
    let mut new_links: Vec<Link> = Vec::new();
    let mut consts: Vec<(Source, u32, SmolStr)> = Vec::new();
    // Links inside the group.
    for l in &inner.links {
        let Some(&to) = map.get(&l.to_node) else {
            continue;
        };
        if input_node(l.from_node) {
            if let Some(src) = group_input(&l.from_socket) {
                consts.push((src, to, l.to_socket.clone()));
            }
        } else if let Some(&from) = map.get(&l.from_node) {
            new_links.push(Link {
                from_node: from,
                from_socket: l.from_socket.clone(),
                to_node: to,
                to_socket: l.to_socket.clone(),
            });
        }
    }
    // Links leaving the group node.
    let mut dead_ends: Vec<Link> = Vec::new();
    for l in t.links.iter().filter(|l| l.from_node == g) {
        match group_output(&l.from_socket) {
            // The add-on's navigation stops on the Group Output: the consumer stays linked to
            // the group node, which is no constant and holds no texture for the patterns.
            Some(Source::Const(..))
                if mode == Mode::Original
                    && inner
                        .link_into(output.unwrap_or(u32::MAX), &l.from_socket)
                        .is_none() =>
            {
                dead_ends.push(l.clone());
            }
            Some(src) => consts.push((src, l.to_node, l.to_socket.clone())),
            None => {}
        }
    }
    match mode {
        Mode::Inline => {
            t.links.retain(|l| l.from_node != g && l.to_node != g);
            t.nodes.retain(|n| n.uid != g);
        }
        // The group node stays (the add-on lists group nodes, `get_material_nodes`: the glTF
        // Material Output is found among them), with its inputs' links and its dead ends.
        Mode::Original => t
            .links
            .retain(|l| l.from_node != g || dead_ends.contains(l)),
    }
    t.nodes.extend(added);
    t.next_node_uid = t.next_node_uid.max(next);
    for (src, to, socket) in consts {
        connect(t, &src, to, &socket, &mut new_links);
    }
    t.links.extend(new_links);
}

/// Rewires the links leaving muted nodes to what the muted node passes
/// (`handle_output_socket__internal_links`), until none is left (chains of muted nodes).
fn bypass_muted(t: &mut NodeTree, mat: &frap_scene::id::Material, images: &dyn ImageLookup) {
    // Each pass rewires one link; a chain of muted nodes needs one pass per node it crosses.
    let guard = (t.links.len() + 1) * (t.nodes.len() + 1);
    for _ in 0..guard {
        let Some(k) = t.links.iter().position(|l| {
            t.node(l.from_node)
                .is_some_and(|n| n.mute && n.bl_idname != "NodeReroute")
        }) else {
            return;
        };
        let l = t.links.remove(k);
        let Some(m) = t.node(l.from_node).cloned() else {
            continue;
        };
        let Some(o) = m.outputs.iter().position(|s| s.identifier == l.from_socket) else {
            continue;
        };
        let internal = {
            let cx = LowerCx::new(t, mat, images);
            crate::graph::fold_internal_input(&cx, &m, o)
        };
        // No internal link, or an unlinked internal input (dangling: the consumer keeps its
        // own value): no link.
        let Some(i) = internal else {
            continue;
        };
        let id = &m.inputs[i].identifier;
        if let Some(src) = t.link_into(m.uid, id).cloned() {
            t.links.push(Link {
                from_node: src.from_node,
                from_socket: src.from_socket,
                to_node: l.to_node,
                to_socket: l.to_socket,
            });
        }
    }
}

/// The tree the exporter reads in place of `tree` when it inlines (`None`: the same as
/// `tree`, no group to inline and no muted node to bypass).
pub fn expand(
    tree: &NodeTree,
    mat: &frap_scene::id::Material,
    images: &dyn ImageLookup,
) -> Option<NodeTree> {
    let has_group = tree.nodes.iter().any(|n| group_of(n, images).is_some());
    let mut t = if has_group {
        flatten(tree, images, MAX_DEPTH, Mode::Inline)
    } else {
        tree.clone()
    };
    let has_muted = t.links.iter().any(|l| {
        t.node(l.from_node)
            .is_some_and(|n| n.mute && n.bl_idname != "NodeReroute")
    });
    if !has_group && !has_muted {
        return None;
    }
    bypass_muted(&mut t, mat, images);
    Some(t)
}

/// The tree the add-on's original-tree readers walk in place of `tree`
/// (`BlenderMaterialIndentifier.__can_use_inline` false: a glTF Material Output with
/// `Occlusion`, `export_unused_textures`, pointer animation): the same group traversal as
/// [`expand`] (`NodeNav.move_back` descends into a group node through its Group Output and
/// ascends from a Group Input, `from_socket` likewise), but no muted-node bypass and no
/// constant folding (the patterns do not fold when [`LowerCx::uses_inline_tree`] is false),
/// inner nodes keeping their names, and an unlinked Group Output input a dead end. `None`
/// when `tree` has no group to expand.
pub fn expand_original(tree: &NodeTree, images: &dyn ImageLookup) -> Option<NodeTree> {
    tree.nodes
        .iter()
        .any(|n| group_of(n, images).is_some())
        .then(|| flatten(tree, images, MAX_DEPTH, Mode::Original))
}

#[cfg(test)]
mod tests {
    use crate::ChannelSel;
    use crate::graph::testkit::*;

    /// A muted node on a texture's path is passed through (its internal link), so the texture
    /// is exported.
    #[test]
    fn muted_nodes_on_a_texture_path_are_passed_through() {
        let mut images = Images::default();
        let img = images.add("albedo", false);
        let mut m = mat("Muted");
        let t = image_node(&mut m, img);
        let gamma = add(&mut m, "ShaderNodeGamma");
        link(&mut m, (t, "Color"), (gamma, "Color"));
        link(&mut m, (gamma, "Color"), (BSDF, "Base Color"));
        m.node_tree.node_mut(gamma).unwrap().mute = true;
        let lm = lower_m(&m, &images);
        let tex = lm
            .base_color
            .texture
            .as_ref()
            .expect("texture through the muted Gamma");
        assert_eq!((tex.image, tex.channel), (img, ChannelSel::Rgb));
    }

    /// A muted node whose internal input is unlinked leaves its consumer with its own value
    /// (dangling): Metallic keeps 0.3.
    #[test]
    fn a_dangling_muted_node_leaves_the_consumer_value() {
        let images = Images::default();
        let mut m = mat("Dangling");
        let math = add(&mut m, "ShaderNodeMath");
        link(&mut m, (math, "Value"), (BSDF, "Metallic"));
        set_in(&mut m, BSDF, "Metallic", float(0.3));
        m.node_tree.node_mut(math).unwrap().mute = true;
        let t = super::expand(&m.node_tree, &m, &images).expect("a muted node to bypass");
        assert!(t.links.iter().all(|l| l.from_node != math));
        assert_eq!(lower_m(&m, &images).metallic.factor, 0.3);
    }

    /// Nothing to inline or bypass: the tree is read as it is.
    #[test]
    fn plain_trees_are_not_copied() {
        let images = Images::default();
        let m = mat("Plain");
        assert!(super::expand(&m.node_tree, &m, &images).is_none());
    }
}
