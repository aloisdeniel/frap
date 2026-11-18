// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.attributes.active_index` (`rna_AttributeGroupID_active_index_get/set`): the active
//! attribute's position in the attribute list (internal attributes included, as Blender
//! counts them), -1 without one. Assigning clamps to `-1 ..= len(attributes)`; an index past
//! the last attribute leaves none active. In Edit Mode frap counts the attributes of the
//! edit-mode view (Blender counts the BMesh layers; D-05).

use frap_rna::traits::FromRnaValue;
use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};

use super::_mesh::{mesh, with_mesh_mut};

/// `AttributeGroupMesh` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "AttributeGroupMesh",
    prop: "active_index",
    coll: false,
};

#[rna_hook(srna = "AttributeGroupMesh", prop = "active_index")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        let i = m
            .attributes_active
            .as_deref()
            .and_then(|n| m.attributes().iter().position(|a| a.name == n))
            .map_or(-1, |i| i as i64);
        Ok(Field::Value(Value::Int(i)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let i = i32::from_rna_value(prop, v)?;
        with_mesh_mut(h, this, |m| {
            let len = m.attributes().len() as i32;
            let i = i.clamp(-1, len);
            m.attributes_active = usize::try_from(i)
                .ok()
                .and_then(|i| m.attributes().iter().nth(i))
                .map(|a| a.name.clone());
        })
    }
}
