// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.nodes.clear()` (`rna_NodeTree_node_clear`): removes every node, and with them
//! every link.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::tree_mut;

#[rna_func(srna = "Nodes", func = "clear")]
fn nodes_clear(h: &mut dyn Host, this: &RnaPtr, _args: &mut FuncArgs) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "clear")?;
    let (main, log) = h.main_mut();
    let t = tree_mut(main, this.root, log)?;
    t.links.clear();
    t.nodes.clear();
    Ok(Value::None)
}
