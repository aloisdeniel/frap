// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.attributes.default_color_name` (`rna_AttributeGroup_default_color_name_get/set`): the name of the render (default)
//! colour attribute, `''` without one. The name is stored as given, even when no colour
//! attribute has it (verified on 5.2.4: `default_color_name = 'Nope'` reads back `'Nope'` and
//! `render_color_index` then reads -1).

use frap_rna::traits::FromRnaValue;
use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{Host, HostRead};
use smol_str::SmolStr;

use super::_mesh::{mesh, with_mesh_mut};

/// `AttributeGroupMesh` is the API struct of a mesh collection (no `rna_struct!` binding of its own),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "AttributeGroupMesh",
    prop: "default_color_name",
    coll: false,
};

#[rna_hook(srna = "AttributeGroupMesh", prop = "default_color_name")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mesh(h, this)?;
        Ok(Field::Value(Value::Str(
            m.color_default.clone().unwrap_or_default(),
        )))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let name = SmolStr::from_rna_value(prop, v)?;
        with_mesh_mut(h, this, |m| {
            m.color_default = (!name.is_empty()).then_some(name);
        })
    }
}
