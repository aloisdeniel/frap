// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeGroup.node_tree = group` (`rna_NodeGroup_node_tree_set` + `node_group_declare`,
//! WP 3.1): the group node instances the group and takes the sockets of its interface (inputs
//! and outputs, values from the items); `None` removes every socket. Links of sockets that no
//! longer exist are removed.
//!
//! Verified on 5.2.4: the group gains one user per group node; assigning the node's own tree,
//! a group that contains it (a cycle) or a tree of another type (`GeometryNodeTree`) is
//! ignored silently (`rna_NodeGroup_node_tree_poll` fails, the pointer keeps its value).

// Ported from blender/source/blender/makesrna/intern/rna_nodetree.cc
// (`rna_NodeGroup_node_tree_set`, `rna_NodeGroup_node_tree_poll`), Blender 5.2.4,
// GPL-2.0-or-later.

use frap_nodes::NodeTreeType;
use frap_rna::{IdAny, NodeTreeId, PropDef, RnaError, RnaPtr, Root, Value, rna_hook};
use frap_scene::Host;

use super::_nodes::{group_reaches, node_index, tree, tree_mut};

/// `ShaderNodeGroup` has a hand-written binding in `frap-nodes` (no `rna_struct!`), so its hook
/// binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ShaderNodeGroup",
    prop: "node_tree",
    coll: false,
};

#[rna_hook(srna = "ShaderNodeGroup", prop = "node_tree")]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let root = this.root;
    let group: Option<IdAny> = match v {
        Value::Id(a) => a,
        Value::Ptr(p) => match (p.root, p.path.is_empty()) {
            (Root::Id(a), true) => Some(a),
            // An embedded tree (a material's or world's): never a group, refused by the poll.
            _ => return Ok(()),
        },
        _ => None,
    };
    let (main, log) = h.main_mut();
    let iface = match group {
        Some(g) => {
            let Some(gt) = g
                .typed::<NodeTreeId>()
                .and_then(|r| main.node_groups.get(r))
            else {
                return Ok(());
            };
            if gt.ty != NodeTreeType::Shader || group_reaches(main, g, root) {
                return Ok(());
            }
            Some(gt.interface.clone())
        }
        None => None,
    };
    let Some(i) = tree(main, root).and_then(|t| node_index(t, &this.path)) else {
        return Ok(());
    };
    let t = tree_mut(main, root, log)?;
    t.nodes[i]
        .props
        .values
        .insert("node_tree", Value::Id(group));
    t.sync_group_node(i, iface.as_deref());
    Ok(())
}
