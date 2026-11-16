// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.children` (Python-only, `bpy_types.py`): the objects whose parent is this one, in
//! `bpy.data.objects` order (verified: `('A_empty', 'b_empty', 'Light')`, a tuple).

use frap_rna::{CollRef, Field, IdRef, ObjectId, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::data::IdMap;
use crate::id::Object;
use crate::id::idlist::IdList;
use crate::id::paths::object_of;

/// Children of `ob` in `bpy.data.objects` order.
pub fn children(objects: &IdMap<Object>, ob: IdRef<ObjectId>) -> Vec<IdRef<ObjectId>> {
    objects
        .iter()
        .filter(|(_, o)| o.parent == Some(ob))
        .map(|(r, _)| r)
        .collect()
}

#[rna_hook(srna = "Object", prop = "children", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let ob = object_of(this)?;
    Ok(Field::Coll(CollRef::Owned(Box::new(IdList::objects(
        children(&h.main().objects, ob),
    )))))
}
