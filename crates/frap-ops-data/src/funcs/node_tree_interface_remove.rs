// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.interface.remove(item, move_content_to_parent=True)` (WP 3.1): removes a socket
//! item; the sockets it declared disappear from the tree's group input/output nodes and from
//! every group node using the group, with their links. Verified on 5.2.4: an item of another
//! tree is ignored silently.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, tree, tree_mut};
use super::node_tree_interface_new_socket::sync_group_users;

#[rna_func(srna = "NodeTreeInterface", func = "remove")]
fn node_tree_interface_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let root = this.root;
    let p = arg_ptr(args, "item").ok_or_else(|| internal("interface.remove(): no item"))?;
    if p.root != root {
        return Ok(Value::None);
    }
    let (main, log) = h.main_mut();
    let Some(i) = frap_nodes::rna::interface::item_index_of(tree(main, root)?, &p.path) else {
        return Ok(Value::None);
    };
    tree_mut(main, root, log)?.interface_remove(i);
    sync_group_users(main, log, root);
    Ok(Value::None)
}
