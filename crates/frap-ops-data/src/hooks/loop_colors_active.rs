// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.vertex_colors.active` (`rna_Mesh_vertex_color_active_get/set`): the active colour
//! attribute when it is a BYTE_COLOR layer on CORNER (a legacy vertex colour layer), `None`
//! otherwise (no active colour, or a FLOAT_COLOR / POINT one). Assigning a layer makes it the
//! active colour attribute; assigning `None` changes nothing. Verified on 5.2.4.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};

use super::_mesh::{assigned_name, item_ptr, loop_color_names, mesh, with_mesh_mut};

/// `LoopColors` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "LoopColors",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "LoopColors", prop = "active")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        let names = loop_color_names(&m);
        Ok(Field::Value(match m.color_active.as_deref() {
            Some(n) if names.iter().any(|x| x == n) => {
                item_ptr(this, n, &frap_spec_rna::MESH_LOOP_COLOR_LAYER)
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
        let name = assigned_name(&v, "LoopColors.active")?;
        with_mesh_mut(h, this, |m| {
            if let Some(n) = name
                && loop_color_names(m).contains(&n)
            {
                m.color_active = Some(n);
            }
        })
    }
}
