// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.uv_layers.active` (`rna_Mesh_uv_layer_active_get/set`): the active UV map, `None`
//! without one. Assigning a layer makes it active; assigning `None` clears it (then
//! `active_index` reads -1). Verified on 5.2.4.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};

use super::_mesh::{assigned_name, item_ptr, mesh, uv_names, with_mesh_mut};

/// `UVLoopLayers` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "UVLoopLayers",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "UVLoopLayers", prop = "active")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        let names = uv_names(&m);
        Ok(Field::Value(match m.uv_active.as_deref() {
            Some(n) if names.iter().any(|x| x == n) => {
                item_ptr(this, n, &frap_spec_rna::MESH_UV_LOOP_LAYER)
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
        let name = assigned_name(&v, "UVLoopLayers.active")?;
        with_mesh_mut(h, this, |m| match name {
            None => m.uv_active = None,
            Some(n) => {
                if uv_names(m).contains(&n) {
                    m.uv_active = Some(n);
                }
            }
        })
    }
}
