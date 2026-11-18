// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.node_groups.remove(tree, do_unlink=True, do_id_user=True, do_ui_user=True)` (WP
//! 3.1): deletes the node group, clearing every reference to it (group nodes keep no
//! `node_tree`); the verified rules, including the zero-users check of `do_unlink=False`, are
//! in `_common.rs`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

#[rna_func(srna = "BlendDataNodeTrees", func = "remove")]
fn blend_data_node_trees_remove(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::remove(h, args, "tree")
}
