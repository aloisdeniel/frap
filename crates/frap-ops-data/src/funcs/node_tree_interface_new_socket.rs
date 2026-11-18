// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.interface.new_socket(name, description="", in_out='INPUT', socket_type='DEFAULT',
//! parent=None)` (`rna_NodeTreeInterfaceItems_new_socket`, WP 3.1): adds a group interface
//! socket and returns it (`….interface.items_tree[i]`).
//!
//! Verified on 5.2.4: identifiers `Socket_<n>` from a per-tree counter; outputs are kept before
//! inputs; 'DEFAULT' is a float socket; the tree's `Group Input`/`Group Output` nodes and every
//! group node using the group get the new socket at once (existing socket values are kept);
//! material and world trees accept items too. What the add-on's `create_settings_group` needs
//! ("glTF Material Output").
//!
//! frap does not store item descriptions or panels yet: a non-empty `description`, a `parent`
//! panel and the `NodeSocketBundle`/`NodeSocketClosure` types raise `NotImplementedError`.

use frap_nodes::{InOut, NodeTree, interface_socket_type};
use frap_rna::{NodeTreeId, RnaError, RnaPtr, Root, Seg, Value, rna_func};
use frap_scene::{ChangeLog, FuncArgs, Host, Main};

use super::_common::{arg_ptr, arg_str, not_implemented, tree_mut, tree_srna};

/// Refreshes the group nodes of every material, world and group tree that instance the group
/// at `root` (no-op for an embedded tree, which no group node can use).
pub(crate) fn sync_group_users(main: &mut Main, log: &mut ChangeLog, root: Root) {
    let Root::Id(gid) = root else {
        return;
    };
    let Some(group) = gid
        .typed::<NodeTreeId>()
        .and_then(|r| main.node_groups.get(r))
    else {
        return;
    };
    let iface = group.interface.clone();
    let uses = |t: &NodeTree| t.nodes.iter().any(|n| n.group_tree() == Some(gid));
    let mats: Vec<_> = main
        .materials
        .iter()
        .filter(|(_, m)| uses(&m.node_tree))
        .map(|(r, _)| r)
        .collect();
    for r in mats {
        if let Some(m) = main.materials.get_mut(r, log) {
            m.node_tree.sync_group_nodes_of(gid, &iface);
        }
    }
    let worlds: Vec<_> = main
        .worlds
        .iter()
        .filter(|(_, w)| uses(&w.node_tree))
        .map(|(r, _)| r)
        .collect();
    for r in worlds {
        if let Some(w) = main.worlds.get_mut(r, log) {
            w.node_tree.sync_group_nodes_of(gid, &iface);
        }
    }
    let groups: Vec<_> = main
        .node_groups
        .iter()
        .filter(|(_, g)| uses(g))
        .map(|(r, _)| r)
        .collect();
    for r in groups {
        if let Some(g) = main.node_groups.get_mut(r, log) {
            g.sync_group_nodes_of(gid, &iface);
        }
    }
}

/// The pointer of interface item `i` of the tree at `root`: `….interface.items_tree[i]`.
pub(crate) fn item_ptr(main: &Main, root: Root, t: &NodeTree, i: usize) -> RnaPtr {
    let srna = frap_nodes::rna::interface::item_srna(t.interface[i].socket_type);
    RnaPtr::new(root, tree_srna(main, root))
        .child(Seg::Prop("interface"), &frap_nodes::rna::interface::NODE_TREE_INTERFACE)
        .child(Seg::Prop("items_tree"), srna)
        .child(Seg::Index(i as i64), srna)
}


#[rna_func(srna = "NodeTreeInterface", func = "new_socket")]
fn node_tree_interface_new_socket(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new_socket")?;
    let name = arg_str(args, "name").to_owned();
    if !arg_str(args, "description").is_empty() {
        return Err(not_implemented(
            "interface socket descriptions are not stored yet; call new_socket() without \
             description",
        ));
    }
    if arg_ptr(args, "parent").is_some() {
        return Err(not_implemented(
            "interface panels are not implemented yet; call new_socket() without parent",
        ));
    }
    let socket_type = arg_str(args, "socket_type").to_owned();
    let ty = interface_socket_type(&socket_type).ok_or_else(|| {
        not_implemented(format!(
            "interface sockets of type {socket_type} are not implemented yet"
        ))
    })?;
    let in_out = match arg_str(args, "in_out") {
        "OUTPUT" => InOut::Output,
        _ => InOut::Input,
    };
    let root = this.root;
    let (main, log) = h.main_mut();
    let t = tree_mut(main, root, log)?;
    let i = t.interface_new_socket(&name, in_out, ty);
    sync_group_users(main, log, root);
    let t = super::_common::tree(main, root)?;
    Ok(Value::Ptr(Box::new(item_ptr(main, root, t, i))))
}
