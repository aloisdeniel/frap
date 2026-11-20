// SPDX-License-Identifier: GPL-3.0-or-later
//! `Key.user` (`Key.from`, WP 6.3): the mesh using the key. frap derives it from the meshes
//! (a key copied with its mesh by `ID.copy()` or a duplicate belongs to the copy, as Blender's
//! `shapekey_copy_data` re-points `from`), falling back to the stored back pointer.

use frap_rna::{Field, IdRef, KeyId, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::id::paths::{internal, root_id};

#[rna_hook(srna = "Key", prop = "user", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let k: IdRef<KeyId> = root_id(this)?
        .typed()
        .ok_or_else(|| internal("Key.user: not a key"))?;
    let main = h.main();
    let key = main
        .shape_keys
        .get(k)
        .ok_or_else(|| internal("Key.user: the key was removed"))?;
    let user = main
        .meshes
        .iter()
        .find(|(_, m)| m.shape_keys == Some(k))
        .map(|(r, _)| r)
        .or(key.user);
    Ok(Field::Id(user.map(IdRef::any)))
}
