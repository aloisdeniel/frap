// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.name` (every datablock): reading gives the header name; assigning renames with
//! Blender's rules (`rna_ID_name_set` → `BKE_id_rename(…, RenameExistingNever)`): a taken name
//! gives the **assigned** ID the lowest free `.NNN` and the other keeps its name; the ID moves
//! to its sorted ListBase position (§5.4). Names are truncated to 255 bytes; an empty name
//! gives the type's default name (verified: `''` → `Material.001` when `Material` exists).
//! Linked IDs are read-only (`AttributeError: bpy_struct: attribute "name" from
//! "GeometryNodeTree" is read-only`, verified). Embedded IDs (`Scene.collection`,
//! `Material.node_tree`) keep their fixed names.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Root, Value, rna_hook};

use crate::data::RenameMode;
use crate::id::paths::internal;
use crate::{Host, HostRead};

#[rna_hook(srna = "ID", prop = "name", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let header = match this.root {
            Root::Id(a) => h.main().header(a),
            Root::Embedded { owner, slot } => h.main().embedded_header(owner, slot),
            _ => None,
        }
        .ok_or_else(|| internal("ID.name: the datablock was removed"))?;
        Ok(Field::Value(Value::Str(header.name.clone())))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let Root::Id(a) = this.root else {
            // Embedded IDs keep their fixed names.
            return Err(frap_rna::traits::read_only(this.srna, prop.identifier));
        };
        let (main, log) = h.main_mut();
        let header = main
            .header(a)
            .ok_or_else(|| internal("ID.name: the datablock was removed"))?;
        if header.library.is_some() {
            return Err(frap_rna::traits::read_only(this.srna, prop.identifier));
        }
        let name = v.as_str().unwrap_or_default().to_owned();
        main.rename(a, &name, RenameMode::Never, log);
        Ok(())
    }
}
