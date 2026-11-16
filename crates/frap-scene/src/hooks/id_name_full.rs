// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.name_full`: the name, followed by ` [<library name>]` for a linked ID (verified:
//! `Smooth by Angle [geometry_nodes_essentials.blend.001]`).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Root, Value, rna_hook};

use crate::HostRead;
use crate::id::paths::internal;

#[rna_hook(srna = "ID", prop = "name_full", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let header = match this.root {
        Root::Id(a) => main.header(a),
        Root::Embedded { owner, slot } => main.embedded_header(owner, slot),
        _ => None,
    }
    .ok_or_else(|| internal("ID.name_full: the datablock was removed"))?;
    let full = match header.library.and_then(|l| main.libraries.get(l)) {
        Some(lib) => format!("{} [{}]", header.name, lib.header.name),
        None => header.name.to_string(),
    };
    Ok(Field::Value(Value::Str(full.into())))
}
