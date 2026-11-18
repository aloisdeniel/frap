// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.uv_layers.active_index` (`rna_Mesh_uv_layer_active_index_get/set`): the active UV
//! map's position in `mesh.uv_layers`, -1 without one. An index outside the layers is ignored
//! (verified on 5.2.4: `= 5` and `= -1` leave the active map unchanged).

use frap_rna::traits::FromRnaValue;
use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};

use super::_mesh::{index_of, mesh, uv_names, with_mesh_mut};

/// `UVLoopLayers` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "UVLoopLayers",
    prop: "active_index",
    coll: false,
};

#[rna_hook(srna = "UVLoopLayers", prop = "active_index")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        Ok(Field::Value(Value::Int(
            index_of(&uv_names(&m), m.uv_active.as_deref()) as i64,
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
                .and_then(|i| uv_names(m).get(i).cloned())
            {
                m.uv_active = Some(n);
            }
        })
    }
}
