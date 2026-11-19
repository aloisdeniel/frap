// SPDX-License-Identifier: GPL-3.0-or-later
//! `NodeTreeInterfaceSocket.name = …` (WP 3.1): renames the item and the sockets it declares,
//! on the group's input/output nodes and on every group node using the group (verified on
//! 5.2.4; identifiers do not change).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::Host;

use super::_common::internal;
use super::_nodes::{sync_group_users, tree, tree_mut};

/// The interface items have hand-written bindings in `frap-nodes` (no `rna_struct!`), so the
/// hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "NodeTreeInterfaceSocket",
    prop: "name",
    coll: false,
};

#[rna_hook(srna = "NodeTreeInterfaceSocket", prop = "name")]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let root = this.root;
    let name = v
        .as_str()
        .ok_or_else(|| internal("interface socket name: not a string"))?
        .to_owned();
    let (main, log) = h.main_mut();
    let Some(i) =
        tree(main, root).and_then(|t| frap_nodes::rna::interface::item_index_of(t, &this.path))
    else {
        return Err(internal("interface socket not found"));
    };
    tree_mut(main, root, log)?.interface_rename(i, &name);
    sync_group_users(main, log, root);
    Ok(())
}
