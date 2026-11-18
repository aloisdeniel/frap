// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.attributes.active` (`rna_AttributeGroupID_active_get/set`): the active attribute,
//! `None` without one; assigning an attribute of the mesh makes it active, `None` clears it.
//! Blender stores an index (`Mesh.attributes_active_index`, see
//! `frap_mesh::Mesh::attributes_active`); the operators that move it (WP 2.9) reproduce the
//! index arithmetic. In Edit Mode frap reads the attribute list of the edit-mode view (D-05).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};

use super::_mesh::{assigned_name, item_ptr, mesh, with_mesh_mut};

/// `AttributeGroupMesh` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "AttributeGroupMesh",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "AttributeGroupMesh", prop = "active")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        Ok(Field::Value(
            match m
                .attributes_active
                .as_deref()
                .and_then(|n| m.attributes().get(n))
            {
                Some(a) => item_ptr(this, &a.name, frap_mesh::attribute_srna(&a.data)),
                None => Value::None,
            },
        ))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let name = assigned_name(&v, "AttributeGroupMesh.active")?;
        with_mesh_mut(h, this, |m| match name {
            None => m.attributes_active = None,
            Some(n) => {
                if m.attributes().get(&n).is_some() {
                    m.attributes_active = Some(n);
                }
            }
        })
    }
}
