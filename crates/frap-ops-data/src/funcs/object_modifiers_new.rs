// SPDX-License-Identifier: GPL-3.0-or-later
//! `obj.modifiers.new(name, type)` (`rna_Object_modifier_new` → `ed::object::modifier_add`,
//! `frap_modifiers::stack::edit::add`): adds a modifier at Blender's position (before the
//! pinned ones; HOOK and `RequiresOriginalData` types after the leading deform-only run), with
//! a unique name (`''` gives the type's name), makes it active and returns it.
//!
//! Verified on 5.2.4: a type the object does not support (`SUBSURF` on a camera,
//! `GREASE_PENCIL_TINT` on a mesh) or a second modifier of a single-instance type (`CLOTH`)
//! returns `None` (Blender's WARNING report is kept in the response); an unknown type is the
//! argument `TypeError`; `type` is required.
//!
//! A type frap has no evaluator for yet is added like any other, with the WARNING
//! `Modifier 'X' (TYPE) is not evaluated by frap; passed through` (§7.2).

use frap_modifiers::ModifierType;
use frap_modifiers::stack::edit;
use frap_rna::{IdRef, ObjectId, Report, ReportType, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, internal, root_id};

#[rna_func(srna = "ObjectModifiers", func = "new")]
fn object_modifiers_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new")?;
    let ob: IdRef<ObjectId> = root_id(this)?
        .typed()
        .ok_or_else(|| internal("modifiers.new(): not an object"))?;
    let name = arg_str(args, "name").to_owned();
    let ty = ModifierType::from_identifier(arg_str(args, "type"))
        .ok_or_else(|| internal("modifiers.new(): type was not converted"))?;
    // Edited on a copy, so a refusal leaves the object untouched (no generation bump).
    let mut o = h
        .main()
        .objects
        .get(ob)
        .ok_or_else(|| internal("modifiers.new(): the object was removed"))?
        .clone();
    match edit::add(&mut o, Some(&name), ty) {
        Ok(i) => {
            let p = o.modifiers[i].ptr(ob.any());
            if frap_modifiers::registry::find(ty).is_none() {
                // §7.2: the modifier is added, but frap passes the mesh through.
                h.report(Report::new(
                    ReportType::Warning,
                    frap_modifiers::stack::pass_through_message(&o.modifiers[i]),
                ));
            }
            let (main, log) = h.main_mut();
            if let Some(dst) = main.objects.get_mut(ob, log) {
                *dst = o;
            }
            Ok(Value::Ptr(Box::new(p)))
        }
        Err(e) => {
            h.report(Report::new(ReportType::Warning, e.to_string()));
            Ok(Value::None)
        }
    }
}
