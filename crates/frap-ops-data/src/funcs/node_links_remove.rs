// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.links.remove(link)` (`rna_NodeTree_link_remove`): removes the link. A link of
//! another tree raises `RuntimeError: Error: Unable to locate link in node tree` (verified on
//! 5.2.4).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, link_of, report_error, tree, tree_mut};

#[rna_func(srna = "NodeLinks", func = "remove")]
fn node_links_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let root = this.root;
    let p = arg_ptr(args, "link").ok_or_else(|| internal("links.remove(): no link argument"))?;
    let (main, log) = h.main_mut();
    let Some(i) = link_of(tree(main, root)?, root, p) else {
        return Err(report_error("Unable to locate link in node tree"));
    };
    tree_mut(main, root, log)?.remove_link(i);
    Ok(Value::None)
}
