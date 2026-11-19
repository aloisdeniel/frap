// SPDX-License-Identifier: GPL-3.0-or-later
//! `obj.modifiers.remove(modifier)` (`rna_Object_modifier_remove` → `ed::object::modifier_remove`,
//! `frap_modifiers::stack::edit::remove`): removes the modifier; when it was the active one,
//! the next one (the previous one at the end of the stack) becomes active. ID users the
//! modifier held (a NODES group, a DISPLACE texture) are released, as the user counts are
//! derived from the data.
//!
//! Verified on 5.2.4: `remove(None)` is the argument `TypeError`; a modifier of another object
//! raises `RuntimeError: Error: Modifier 'X' not in object 'OBCube'` (Blender prints the ID
//! name with its `OB` prefix).

use frap_modifiers::stack::edit;
use frap_rna::{IdRef, ObjectId, RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::id::Modifier;
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, report_error, root_id};

#[rna_func(srna = "ObjectModifiers", func = "remove")]
fn object_modifiers_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let ob: IdRef<ObjectId> = root_id(this)?
        .typed()
        .ok_or_else(|| internal("modifiers.remove(): not an object"))?;
    let p = arg_ptr(args, "modifier")
        .ok_or_else(|| internal("modifiers.remove(): modifier was not converted"))?;
    let main = h.main();
    let o = main
        .objects
        .get(ob)
        .ok_or_else(|| internal("modifiers.remove(): the object was removed"))?;
    let same = root_id(p).ok() == Some(ob.any());
    let i = same.then(|| Modifier::index_in(&o.modifiers, &p.path)).flatten();
    let Some(i) = i else {
        let name = p
            .path
            .iter()
            .rev()
            .find_map(|s| match s {
                Seg::Key(k) => Some(k.to_string()),
                _ => None,
            })
            .unwrap_or_default();
        return Err(report_error(format!(
            "Modifier '{name}' not in object 'OB{}'",
            o.header.name
        )));
    };
    let (main, log) = h.main_mut();
    if let Some(o) = main.objects.get_mut(ob, log) {
        edit::remove(&mut o.modifiers, i);
    }
    Ok(Value::None)
}
