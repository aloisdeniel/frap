// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.interface.clear()` (WP 3.1): removes every item; the group input/output nodes
//! keep only their `__extend__` socket and the group nodes using the group lose every socket
//! and link (verified on 5.2.4).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::tree_mut;
use super::node_tree_interface_new_socket::sync_group_users;

#[rna_func(srna = "NodeTreeInterface", func = "clear")]
fn node_tree_interface_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "clear")?;
    let root = this.root;
    let (main, log) = h.main_mut();
    tree_mut(main, root, log)?.interface_clear();
    sync_group_users(main, log, root);
    Ok(Value::None)
}
