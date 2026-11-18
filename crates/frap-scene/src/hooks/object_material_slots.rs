// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.material_slots`: derived from `data.materials` and the object's overrides (§5.6).

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::id::paths::{internal, object_of};
use crate::id::slots::MaterialSlots;

#[rna_hook(srna = "Object", prop = "material_slots", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let ob = main
        .objects
        .get(object_of(this)?)
        .ok_or_else(|| internal("Object.material_slots: the object was removed"))?;
    Ok(Field::Coll(CollRef::Owned(Box::new(MaterialSlots {
        main,
        ob,
    }))))
}
