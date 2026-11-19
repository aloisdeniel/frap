// SPDX-License-Identifier: GPL-3.0-or-later
//! `NodeTreeInterfaceSocket.socket_type = …` (`rna_NodeTreeInterfaceSocket_socket_type_set` +
//! `bNodeTreeInterfaceSocket::set_socket_type`, WP 3.1): the item takes the new type and the
//! fresh value of that type; the sockets it declares on the group's input/output nodes and on
//! every group node using the group change type, take the item's value and keep their links.
//!
//! Verified on 5.2.4: the items depend on the tree type (a shader tree refuses
//! `NodeSocketGeometry` with `TypeError: bpy_struct: item.attr = val: enum "NodeSocketGeometry"
//! not found in ('NodeSocketFloat', …, 'NodeSocketClosure')`; 'DEFAULT' is refused too); writing
//! the current type also resets the item's value. Types frap cannot store as interface items
//! yet raise `NotImplementedError`.

// Ported from blender/source/blender/makesrna/intern/rna_node_tree_interface.cc
// (`rna_NodeTreeInterfaceSocket_socket_type_set`, `..._socket_type_itemf`) and
// blender/source/blender/blenkernel/intern/node_tree_interface.cc (`set_socket_type`),
// Blender 5.2.4, GPL-2.0-or-later.

use frap_nodes::rna::interface::{SET_PREFIX, item_index_of, socket_type_of};
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
    prop: "socket_type",
    coll: false,
};

#[rna_hook(srna = "NodeTreeInterfaceSocket", prop = "socket_type")]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let root = this.root;
    let Value::Enum(name) = v else {
        return Err(internal("interface socket_type: not an enum"));
    };
    let (main, log) = h.main_mut();
    let (i, tree_ty) = tree(main, root)
        .and_then(|t| Some((item_index_of(t, &this.path)?, t.ty)))
        .ok_or_else(|| internal("interface socket not found"))?;
    let ty = socket_type_of(tree_ty, &name, SET_PREFIX)?;
    tree_mut(main, root, log)?.interface_set_socket_type(i, ty);
    sync_group_users(main, log, root);
    Ok(())
}
