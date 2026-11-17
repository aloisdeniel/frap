// SPDX-License-Identifier: GPL-3.0-or-later
//! `collection.children.unlink(child)` (`rna_Collection_children_unlink`): removes a direct
//! child collection. A collection that is not a direct child raises `RuntimeError: Error:
//! Collection 'K' not in collection 'Scene Collection'` (verified on 5.2.4). View layers are
//! resynced at once.
//!
//! `unlink(None)` crashes Blender 5.2.4; frap raises a `TypeError` instead.

use frap_rna::{CollectionId, IdRef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_func};
use frap_scene::id::layers::sync_all;
use frap_scene::{FuncArgs, Host};

use super::_common::{CollTarget, arg_id, report_error};

#[rna_func(srna = "CollectionChildren", func = "unlink")]
fn collection_children_unlink(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "unlink")?;
    let target = CollTarget::of(this)?;
    let (main, log) = h.main_mut();
    let c = target.get(main)?;
    let child: Option<IdRef<CollectionId>> = arg_id(args, "child").and_then(|a| a.typed());
    let Some(child) = child else {
        return Err(RnaError::new(
            RnaErrorKind::FuncArgs,
            PyExc::TypeError,
            "CollectionChildren.unlink(): frap: child is None or not a bpy.data collection \
             (Blender 5.2.4 crashes on None)",
        ));
    };
    if !c.children.contains(&child) {
        return Err(report_error(format!(
            "Collection '{}' not in collection '{}'",
            main.collections.name_of(child).unwrap_or_default(),
            c.header.name
        )));
    }
    target.get_mut(main, log)?.children.retain(|x| *x != child);
    sync_all(main, log);
    Ok(Value::None)
}
