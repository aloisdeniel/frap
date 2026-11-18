// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.attributes.active_color` / `mesh.color_attributes.active_color`
//! (`rna_AttributeGroup_active_color_get/set`): the active colour attribute, `None` without
//! one. Assigning a colour attribute makes it active, `None` clears it. Verified on 5.2.4.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};

use super::_mesh::{assigned_name, color_names, item_ptr, mesh, with_mesh_mut};

/// `AttributeGroupMesh` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "AttributeGroupMesh",
    prop: "active_color",
    coll: false,
};

#[rna_hook(srna = "AttributeGroupMesh", prop = "active_color")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        let names = color_names(&m);
        Ok(Field::Value(match m.color_active.as_deref() {
            Some(n) if names.iter().any(|x| x == n) => {
                let srna = m
                    .attributes()
                    .get(n)
                    .map_or(&frap_spec_rna::ATTRIBUTE, |a| {
                        frap_mesh::attribute_srna(&a.data)
                    });
                item_ptr(this, n, srna)
            }
            _ => Value::None,
        }))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let name = assigned_name(&v, "AttributeGroupMesh.active_color")?;
        with_mesh_mut(h, this, |m| match name {
            None => m.color_active = None,
            Some(n) => {
                if color_names(m).contains(&n) {
                    m.color_active = Some(n);
                }
            }
        })
    }
}
