// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.users_collection` (Python-only, `bpy_types.py`): the collections of
//! `bpy.data.collections` that link the object, then the master collections of the scenes
//! that link it directly (verified: `(bpy.data.collections['Collection'],
//! bpy.data.collections['Other'])` and `(bpy.data.scenes['Scene'].collection,)`).
//!
//! A tuple value: a master collection is embedded (no `IdAny`), so it is a pointer item
//! (`Value::Ptr` at `Root::Embedded`), which a collection of IDs cannot hold.

use frap_rna::{
    EmbeddedSlot, Field, PropDef, RnaError, RnaPtr, Root, Value, rna_hook,
};

use crate::HostRead;
use crate::id::paths::object_of;

#[rna_hook(srna = "Object", prop = "users_collection", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let ob = object_of(this)?;
    let main = h.main();
    let mut out: Vec<Value> = main
        .collections
        .iter()
        .filter(|(_, c)| c.objects.contains(&ob))
        .map(|(r, _)| Value::Id(Some(r.any())))
        .collect();
    for (r, s) in main.scenes.iter() {
        if s.collection.objects.contains(&ob) {
            out.push(Value::Ptr(Box::new(RnaPtr::new(
                Root::Embedded {
                    owner: r.any(),
                    slot: EmbeddedSlot::MasterCollection,
                },
                &frap_spec_rna::COLLECTION,
            ))));
        }
    }
    Ok(Field::Value(Value::Tuple(out)))
}
