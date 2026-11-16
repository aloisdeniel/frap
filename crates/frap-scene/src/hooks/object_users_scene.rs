// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.users_scene` (Python-only, `bpy_types.py`): the scenes whose `objects` contain the
//! object, in `bpy.data.scenes` order (verified: `(bpy.data.scenes['S2'],
//! bpy.data.scenes['Scene'])`, a `tuple`), returned as a tuple value.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::HostRead;
use crate::hooks::scene_objects::scene_objects;
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
        .map(|(r, _)| Value::from(r))
        .collect();
    Ok(Field::Value(Value::Tuple(scenes)))
}
