// SPDX-License-Identifier: GPL-3.0-or-later
//! `VertexGroup.lock_weight` writes (`bDeformGroup.flag & DG_LOCK_WEIGHT`, WP 6.3): the lock
//! lives on the mesh with the group names (`Mesh::vertex_group_locks`). Locked groups are kept
//! by `object.vertex_group_remove(all_unlocked=True)`, keep their weights in
//! `object.vertex_group_normalize_all`, and make `object.vertex_group_assign` /
//! `vertex_group_remove_from` fail their poll when active ("The active vertex group is
//! locked"), as in Blender 5.2.4.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::object_vertex_groups::{group_of, object_mesh};
use crate::Host;
use crate::id::paths::internal;

/// `VertexGroup` items are proxies (`hooks/object_vertex_groups.rs`), without a binding.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "VertexGroup",
    prop: "lock_weight",
    coll: false,
};

#[rna_hook(srna = "VertexGroup", prop = "lock_weight", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (ob, i) = group_of(h.main(), this)?;
    let me =
        object_mesh(h.main(), ob).ok_or_else(|| internal("VertexGroup.lock_weight: no mesh"))?;
    let locked = v.as_bool().unwrap_or(false);
    let (main, log) = h.main_mut();
    if main
        .meshes
        .get(me)
        .is_some_and(|m| m.vertex_group_locked(i) != locked)
        && let Some(m) = main.meshes.get_mut(me, log)
    {
        m.set_vertex_group_locked(i, locked);
    }
    Ok(())
}
