// SPDX-License-Identifier: GPL-3.0-or-later
//! `node_tree.links.new(input, output, verify_limits=True, handle_dynamic_sockets=False)`
//! (`rna_NodeTree_link_new` → `NodeTree::link_new`, WP 1.11): links two sockets (swapped when
//! given input first) and returns the new link, appended last. With `verify_limits` every
//! link into the target input is replaced. Verified on 5.2.4: two sockets of the same
//! direction raise `RuntimeError: Error: Same input/output direction of sockets`; a socket of
//! another tree makes the call return `None` without linking anything.
//! `handle_dynamic_sockets` only matters for group-interface sockets (M3) and is ignored.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{
    arg_bool, arg_ptr, as_report, internal, link_ptr, socket_of, tree, tree_mut,
};

#[rna_func(srna = "NodeLinks", func = "new")]
fn node_links_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let root = this.root;
    let a = arg_ptr(args, "input").ok_or_else(|| internal("links.new(): no input socket"))?;
    let b = arg_ptr(args, "output").ok_or_else(|| internal("links.new(): no output socket"))?;
    let verify = arg_bool(args, "verify_limits", true);
    let (main, log) = h.main_mut();
    let t = tree(main, root)?;
    let (Some(a), Some(b)) = (socket_of(t, root, a), socket_of(t, root, b)) else {
        return Ok(Value::None);
    };
    let i = tree_mut(main, root, log)?
        .link_new(a, b, verify)
        .map_err(as_report)?;
    Ok(Value::Ptr(Box::new(link_ptr(main, root, i))))
}
