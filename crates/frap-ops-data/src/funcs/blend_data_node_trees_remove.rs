// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.node_groups.remove(tree, do_unlink=True, do_id_user=True, do_ui_user=True)` (WP
//! 3.1): deletes the node group, clearing every reference to it (group nodes keep no
//! `node_tree`); the verified rules, including the zero-users check of `do_unlink=False`, are
//! in `_common.rs`.
//!
//! Verified on 5.2.4: a group node that used the group loses every socket, with their links,
//! as after `node.node_tree = None` (`node.inputs[0]` is an `IndexError`).

use frap_nodes::NodeTree;
use frap_rna::{IdAny, MaterialId, NodeTreeId, RnaError, RnaPtr, Value, WorldId, rna_func};
use frap_scene::{FuncArgs, Host, Main};

use super::_common::arg_id;

/// A node tree that holds group nodes of the removed group, with their node uids.
enum User {
    Material(frap_rna::IdRef<MaterialId>, Vec<u32>),
    World(frap_rna::IdRef<WorldId>, Vec<u32>),
    Group(frap_rna::IdRef<NodeTreeId>, Vec<u32>),
}

/// The uids of the group nodes of `t` that instance `gid`.
fn group_nodes(t: &NodeTree, gid: IdAny) -> Vec<u32> {
    t.nodes
        .iter()
        .filter(|n| n.group_tree() == Some(gid))
        .map(|n| n.uid)
        .collect()
}

fn users(main: &Main, gid: IdAny) -> Vec<User> {
    let mut out = Vec::new();
    for (r, m) in main.materials.iter() {
        let uids = group_nodes(&m.node_tree, gid);
        if !uids.is_empty() {
            out.push(User::Material(r, uids));
        }
    }
    for (r, w) in main.worlds.iter() {
        let uids = group_nodes(&w.node_tree, gid);
        if !uids.is_empty() {
            out.push(User::World(r, uids));
        }
    }
    for (r, g) in main.node_groups.iter() {
        let uids = group_nodes(g, gid);
        if r.any() != gid && !uids.is_empty() {
            out.push(User::Group(r, uids));
        }
    }
    out
}

/// Drops the sockets (and their links) of the group nodes `uids` of `t`, whose `node_tree` the
/// removal cleared (`node_group_declare` with no tree).
fn clear_group_nodes(t: &mut NodeTree, uids: &[u32]) {
    for &uid in uids {
        if let Some(i) = t.node_index(uid) {
            t.sync_group_node(i, None);
        }
    }
}

#[rna_func(srna = "BlendDataNodeTrees", func = "remove")]
fn blend_data_node_trees_remove(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let users = match arg_id(args, "tree") {
        Some(gid) => users(h.main(), gid),
        None => Vec::new(),
    };
    super::_common::remove(h, args, "tree")?;
    let (main, log) = h.main_mut();
    for u in users {
        match u {
            User::Material(r, uids) => {
                if let Some(m) = main.materials.get_mut(r, log) {
                    clear_group_nodes(&mut m.node_tree, &uids);
                }
            }
            User::World(r, uids) => {
                if let Some(w) = main.worlds.get_mut(r, log) {
                    clear_group_nodes(&mut w.node_tree, &uids);
                }
            }
            User::Group(r, uids) => {
                if let Some(g) = main.node_groups.get_mut(r, log) {
                    clear_group_nodes(g, &uids);
                }
            }
        }
    }
    Ok(Value::None)
}
