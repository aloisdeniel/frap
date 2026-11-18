// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.nodes.new(type)` on a material's or world's tree or a node group
//! (`rna_NodeTree_node_new` → `NodeTree::new_node`, WP 1.11): appends a fresh node named after
//! its type, made unique with `.001` (`Math`, `Math.001`), at (0, 0), and returns it. Verified
//! on 5.2.4: an unknown type raises `RuntimeError: Error: Node type Foo undefined`; a type of
//! another tree type raises `RuntimeError: Error: Cannot add node of type GeometryNodeMeshCube
//! to node tree 'Shader Nodetree'` plus the poll message.
//! A `Group Input` / `Group Output` node gets the sockets of the tree's interface (WP 3.1,
//! `frap_nodes::NodeTree::sync_group_io_nodes`); a group node has none until its `node_tree`
//! is set.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, as_report, node_ptr, tree, tree_mut};

#[rna_func(srna = "Nodes", func = "new")]
fn nodes_new(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new")?;
    let root = this.root;
    let ty = arg_str(args, "type").to_owned();
    let (main, log) = h.main_mut();
    let t = tree_mut(main, root, log)?;
    let i = t.new_node(&ty).map_err(as_report)?;
    if matches!(
        t.nodes[i].bl_idname,
        frap_nodes::GROUP_INPUT | frap_nodes::GROUP_OUTPUT
    ) {
        t.sync_group_io_nodes();
    }
    let p = node_ptr(main, root, tree(main, root)?, i);
    Ok(Value::Ptr(Box::new(p)))
}
