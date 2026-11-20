// SPDX-License-Identifier: GPL-3.0-or-later
//! `Armature.is_editmode` (WP 6.2, `rna_Armature_is_editmode_get`): whether the armature has
//! edit bones (an object using it is in Edit Mode).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::HostRead;
use crate::edit_armature::paths::armature_of;

#[rna_hook(srna = "Armature", prop = "is_editmode", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let arm = armature_of(this)?;
    Ok(Field::Value(Value::Bool(
        h.edit().armatures.contains_key(&arm.key()),
    )))
}
