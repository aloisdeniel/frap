// SPDX-License-Identifier: GPL-3.0-or-later
//! `VertexGroup.name` (`rna_VertexGroup_name_set` → `BKE_object_defgroup_set_name`, WP 6.3):
//! unique among the mesh's vertex groups (`BLI_uniquename_cb`, default `Group`, 63 bytes).
//! Verified on 5.2.4: renaming `Group.001` to the taken `Group` gives `Group.001` again.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::object_vertex_groups::{group_of, object_mesh};
use crate::Host;
use crate::id::paths::internal;

/// `VertexGroup` items are proxies (`hooks/object_vertex_groups.rs`), without a binding.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "VertexGroup",
    prop: "name",
    coll: false,
};

#[rna_hook(srna = "VertexGroup", prop = "name", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (ob, i) = group_of(h.main(), this)?;
    let me = object_mesh(h.main(), ob).ok_or_else(|| internal("VertexGroup.name: no mesh"))?;
    let requested = v.as_str().unwrap_or_default().to_owned();
    let (main, log) = h.main_mut();
    let names = main
        .meshes
        .get(me)
        .map(|m| m.vertex_group_names.clone())
        .unwrap_or_default();
    let name = frap_bmesh::shape::deform::unique_group_name(&names, Some(i), &requested);
    if let Some(m) = main.meshes.get_mut(me, log)
        && let Some(n) = m.vertex_group_names.get_mut(i)
    {
        *n = name;
    }
    Ok(())
}
