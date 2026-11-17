// SPDX-License-Identifier: GPL-3.0-or-later
//! `collection.objects.unlink(object)` (`rna_Collection_objects_unlink`): removes the object
//! from the collection (one user less). An object that is not in it raises `RuntimeError:
//! Error: Object 'E' not in collection 'Collection'` (verified on 5.2.4). View layers are
//! resynced at once (an object left in no visible collection loses its base and stops being
//! active).
//!
//! `unlink(None)` crashes Blender 5.2.4 (`rna_collection_objects_edit_check` dereferences the
//! null object); frap raises a `TypeError` instead.

use frap_rna::{IdRef, ObjectId, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_func};
use frap_scene::id::layers::sync_all;
use frap_scene::{FuncArgs, Host};

use super::_common::{CollTarget, arg_id, object_name, report_error};

#[rna_func(srna = "CollectionObjects", func = "unlink")]
fn collection_objects_unlink(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let target = CollTarget::of(this)?;
    let ob: IdRef<ObjectId> = arg_id(args, "object")
        .and_then(|a| a.typed())
        .ok_or_else(|| {
            RnaError::new(
                RnaErrorKind::FuncArgs,
                PyExc::TypeError,
                "CollectionObjects.unlink(): frap: object is None (Blender 5.2.4 crashes here)",
            )
        })?;
    let (main, log) = h.main_mut();
    let c = target.get(main)?;
    if !c.objects.contains(&ob) {
        return Err(report_error(format!(
            "Object '{}' not in collection '{}'",
            object_name(main, ob),
            c.header.name
        )));
    }
    target.get_mut(main, log)?.objects.retain(|o| *o != ob);
    sync_all(main, log);
    Ok(Value::None)
}
