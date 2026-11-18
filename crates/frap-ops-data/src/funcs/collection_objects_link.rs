// SPDX-License-Identifier: GPL-3.0-or-later
//! `collection.objects.link(object)` (`rna_Collection_objects_link`), on a
//! `bpy.data.collections` ID or a scene's master collection: appends the object (one more
//! user). An object already in the collection raises `RuntimeError: Error: Object 'Cube'
//! already in collection 'Collection'` (verified on 5.2.4). View layers are resynced at once
//! (Blender syncs lazily, `frap_scene::id::layers`); a new base is selected when the object's
//! stored base flag is (`object_base_new`, e.g. a selected object linked into another scene).

use frap_rna::{IdRef, ObjectId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::layers::sync_all;
use frap_scene::{FuncArgs, Host};

use super::_common::{CollTarget, arg_id, internal, object_name, report_error};

#[rna_func(srna = "CollectionObjects", func = "link")]
fn collection_objects_link(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "link")?;
    let target = CollTarget::of(this)?;
    let ob: IdRef<ObjectId> = arg_id(args, "object")
        .and_then(|a| a.typed())
        .ok_or_else(|| internal("objects.link(): no object argument"))?;
    let (main, log) = h.main_mut();
    let c = target.get(main)?;
    if c.objects.contains(&ob) {
        return Err(report_error(format!(
            "Object '{}' already in collection '{}'",
            object_name(main, ob),
            c.header.name
        )));
    }
    target.get_mut(main, log)?.objects.push(ob);
    sync_all(main, log);
    Ok(Value::None)
}
