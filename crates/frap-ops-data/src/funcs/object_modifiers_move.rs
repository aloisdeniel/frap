// SPDX-License-Identifier: GPL-3.0-or-later
//! `obj.modifiers.move(from_index, to_index)` (`rna_Object_modifier_move` →
//! `ed::object::modifier_move_to_index(…, RPT_ERROR, allow_partial = false)`,
//! `frap_modifiers::stack::edit::move_to_index`).
//!
//! Verified on 5.2.4: `move(99, 2)` raises `RuntimeError: Error: Invalid original modifier
//! index '99'`; `move(2, 99)` and `move(2, -1)` raise `Error: Cannot move modifier beyond the end
//! of the stack`; the move checks of `object_modifier_check_move_before/after` raise their
//! messages, and a pinned modifier blocking an unpinned one refuses silently (nothing moves).

use frap_modifiers::stack::edit;
use frap_rna::{IdRef, ObjectId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_int, internal, report_error, root_id};

#[rna_func(srna = "ObjectModifiers", func = "move")]
fn object_modifiers_move(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "move")?;
    let ob: IdRef<ObjectId> = root_id(this)?
        .typed()
        .ok_or_else(|| internal("modifiers.move(): not an object"))?;
    let from = arg_int(args, "from_index", -1);
    let to = arg_int(args, "to_index", -1);
    let mut mods = h
        .main()
        .objects
        .get(ob)
        .ok_or_else(|| internal("modifiers.move(): the object was removed"))?
        .modifiers
        .clone();
    let Some(i) = usize::try_from(from).ok().filter(|i| *i < mods.len()) else {
        return Err(report_error(format!(
            "Invalid original modifier index '{from}'"
        )));
    };
    match edit::move_to_index(&mut mods, i, to, false) {
        Ok(()) => {
            let changed = h
                .main()
                .objects
                .get(ob)
                .is_some_and(|o| o.modifiers != mods);
            let (main, log) = h.main_mut();
            if changed && let Some(o) = main.objects.get_mut(ob, log) {
                o.modifiers = mods;
            }
            Ok(Value::None)
        }
        Err(Some(msg)) => Err(report_error(msg)),
        Err(None) => Ok(Value::None),
    }
}
