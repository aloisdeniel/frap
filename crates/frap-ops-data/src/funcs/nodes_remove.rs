// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.nodes.remove(node)` (`rna_NodeTree_node_remove` → `NodeTree::remove_node`):
//! removes the node and every link touching it; children of a removed frame keep their place
//! (WP 1.11). A node of another tree raises `RuntimeError: Error: Unable to locate node
//! 'World Output' in node tree` (verified on 5.2.4).

use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, node_of, report_error, tree, tree_mut};

#[rna_func(srna = "Nodes", func = "remove")]
fn nodes_remove(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let root = this.root;
    let p = arg_ptr(args, "node").ok_or_else(|| internal("nodes.remove(): no node argument"))?;
    let (main, log) = h.main_mut();
    let Some(i) = node_of(tree(main, root)?, root, p) else {
        // The node's own name, read from its pointer (`nodes["Name"]`).
        let name = match p.path.last() {
            Some(Seg::Key(k)) => k.to_string(),
            _ => String::new(),
        };
        return Err(report_error(format!(
            "Unable to locate node '{name}' in node tree"
        )));
    };
    tree_mut(main, root, log)?.remove_node(i);
    Ok(Value::None)
}
