// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.attributes.active_color_index` (`rna_AttributeGroup_active_color_index_get/set`): the position of the
//! active colour attribute in `mesh.color_attributes`, -1 without one. An index outside the
//! colour attributes is ignored. Verified on 5.2.4.

use frap_rna::traits::FromRnaValue;
use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};

use super::_mesh::{color_names, index_of, mesh, with_mesh_mut};

/// `AttributeGroupMesh` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "AttributeGroupMesh",
    prop: "active_color_index",
    coll: false,
};

#[rna_hook(srna = "AttributeGroupMesh", prop = "active_color_index")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        Ok(Field::Value(Value::Int(
            index_of(&color_names(&m), m.color_active.as_deref()) as i64,
        )))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let i = i32::from_rna_value(prop, v)?;
        with_mesh_mut(h, this, |m| {
            if let Some(n) = usize::try_from(i)
                .ok()
                .and_then(|i| color_names(m).get(i).cloned())
            {
                m.color_active = Some(n);
            }
        })
    }
}
