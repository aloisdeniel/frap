// SPDX-License-Identifier: GPL-3.0-or-later
//! `obj.modifiers.clear()` (`rna_Object_modifier_clear` → `ed::object::modifiers_clear`):
//! removes every modifier; on an object without modifiers it changes nothing (verified on
//! 5.2.4: a second `clear()` succeeds).

use frap_rna::{IdRef, ObjectId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, root_id};

#[rna_func(srna = "ObjectModifiers", func = "clear")]
fn object_modifiers_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "clear")?;
    let ob: IdRef<ObjectId> = root_id(this)?
        .typed()
        .ok_or_else(|| internal("modifiers.clear(): not an object"))?;
    let empty = h
        .main()
        .objects
        .get(ob)
        .ok_or_else(|| internal("modifiers.clear(): the object was removed"))?
        .modifiers
        .is_empty();
    if !empty {
        let (main, log) = h.main_mut();
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.modifiers.clear();
        }
    }
    Ok(Value::None)
}
