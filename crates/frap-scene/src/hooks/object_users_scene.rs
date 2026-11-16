// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.users_scene` (Python-only, `bpy_types.py`): the scenes whose `objects` contain the
//! object, in `bpy.data.scenes` order (verified: `(bpy.data.scenes['S2'],
//! bpy.data.scenes['Scene'])`).

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::hooks::scene_objects::scene_objects;
use crate::id::idlist::IdList;
use crate::id::paths::object_of;

#[rna_hook(srna = "Object", prop = "users_scene", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let ob = object_of(this)?;
    let main = h.main();
    let scenes = main
        .scenes
        .iter()
        .filter(|(_, s)| scene_objects(main, s).contains(&ob))
        .map(|(r, _)| r)
        .collect();
    Ok(Field::Coll(CollRef::Owned(Box::new(IdList::new(
        scenes,
        &frap_spec_rna::SCENE,
    )))))
}
