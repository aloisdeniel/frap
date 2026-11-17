// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.links.clear()` (`rna_NodeTree_link_clear`): removes every link.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::tree_mut;

#[rna_func(srna = "NodeLinks", func = "clear")]
fn node_links_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let (main, log) = h.main_mut();
    tree_mut(main, this.root, log)?.links.clear();
    Ok(Value::None)
}
