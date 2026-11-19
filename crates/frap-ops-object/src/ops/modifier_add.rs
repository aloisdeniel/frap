// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_add(type='SUBSURF', use_selected_objects=False)`, tier T1. Port of
//! `modifier_add_exec`, `modifier_add_itemf` and `ed::object::modifier_add`
//! (`editors/object/object_modifier.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `frap_modifiers::stack::edit::add`.
//!
//! Verified on 5.2.4: adds to the active object (every selected editable object with the hidden
//! `use_selected_objects`), in Object and Edit Mode; the new modifier is active; `type` items
//! follow the active object (a camera has none: `enum "SUBSURF" not found in ()`; grease-pencil
//! types and `SURFACE` are not offered for meshes); a second single-instance modifier (`CLOTH`)
//! is `CANCELLED` with the WARNING `Only one modifier of this type is allowed`; without an active
//! object the poll fails with `Context missing active object`.
//!
//! A type frap has no evaluator for yet is added, with the WARNING `Modifier 'X' (TYPE) is not
//! evaluated by frap; passed through` (§7.2).
//!
//! This file also holds the helpers the `object.modifier_*` operators share ([`shared`]).

use frap_modifiers::ModifierType;
use frap_modifiers::registry;
use frap_modifiers::stack::{self, edit};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollFail, PollReq};
use frap_rna::{PropKind, PyExc, RnaError, RnaErrorKind};
use frap_scene::Context;
use frap_spec_ops_object::object;

/// Helpers shared by the `object.modifier_*` operators (`edit_modifier_poll_generic`,
/// `modifier_get_edit_objects`, `ed_operator_object_active_editable_ex`).
pub(crate) mod shared {
    use frap_op::{PollFail, PollReq, poll};
    use frap_rna::{IdRef, ObjectId};
    use frap_scene::Context;
    use frap_scene::id::Object;
    use smallvec::smallvec;

    /// `Operator bpy.ops.object.<op>.poll() <why>`.
    pub(crate) fn poll_message(op: &str, why: &str) -> String {
        format!("Operator bpy.ops.object.{op}.poll() {why}")
    }

    /// `ED_operator_object_active_editable_ex` with its messages (`Context missing active
    /// object`, `Cannot edit library linked object`, `Cannot edit hidden object`).
    pub(crate) fn active_editable(c: &Context, op: &str) -> Result<(), PollFail> {
        poll::active_editable(c).map_err(|mut f| {
            let why = match poll::active(c) {
                None => "Context missing active object",
                Some((_, ob)) if ob.header.library.is_some() => "Cannot edit library linked object",
                Some(_) => "Cannot edit hidden object",
            };
            f.message = Some(poll_message(op, why));
            f
        })
    }

    /// `edit_modifier_poll_generic(C, RNA_Modifier, 0, is_editmode_allowed, false)`: an active
    /// (possibly hidden) local object; outside Edit Mode unless allowed (`This modifier
    /// operation is not allowed from Edit mode`). frap has no `modifier` context pointer, so
    /// the object is always the active one.
    pub(crate) fn edit_modifier_poll(
        c: &Context,
        op: &str,
        editmode_allowed: bool,
    ) -> Result<(), PollFail> {
        let Some((_, ob)) = poll::active(c) else {
            return Err(PollFail {
                message: None,
                unmet: smallvec![PollReq::ActiveObject],
                hint: Some("bpy.context.view_layer.objects.active = bpy.data.objects[…]".into()),
            });
        };
        if ob.header.library.is_some() {
            return Err(PollFail {
                message: None,
                unmet: smallvec![PollReq::ActiveEditable],
                hint: None,
            });
        }
        if !editmode_allowed && poll::edit_object(c).is_some() {
            return Err(PollFail {
                message: Some(poll_message(
                    op,
                    "This modifier operation is not allowed from Edit mode",
                )),
                unmet: smallvec![PollReq::ModeObject],
                hint: Some("bpy.ops.object.mode_set(mode='OBJECT')".into()),
            });
        }
        Ok(())
    }

    /// `modifier_get_edit_objects`: the selected editable objects with `use_selected_objects`,
    /// else the active object.
    pub(crate) fn edit_objects(c: &Context, use_selected: bool) -> Vec<IdRef<ObjectId>> {
        if use_selected {
            c.selected_editable_objects()
        } else {
            c.active_object().into_iter().collect()
        }
    }

    /// `BKE_modifiers_findby_name`.
    pub(crate) fn find(o: &Object, name: &str) -> Option<usize> {
        o.modifiers.iter().position(|m| m.name == name)
    }

    /// frap: Blender cancels a modifier operator whose `modifier` names no modifier of the
    /// edited objects without a message; an agent gets a WARNING naming the objects and the
    /// closest modifier names (`frap_rna::error::suggest`), or the ones that exist.
    pub(crate) fn report_missing(cx: &mut frap_op::OpCtx, objects: &[IdRef<ObjectId>], name: &str) {
        let main = cx.main();
        let mut obs: Vec<String> = Vec::new();
        let mut cands: Vec<String> = Vec::new();
        for ob in objects {
            if let Some(o) = main.objects.get(*ob) {
                obs.push(format!("'{}'", o.header.name));
                cands.extend(o.modifiers.iter().map(|m| m.name.to_string()));
            }
        }
        let owner = match obs.as_slice() {
            [] => return,
            [one] => format!("object {one}"),
            many => format!("the edited objects ({})", many.join(", ")),
        };
        let sugg = frap_rna::error::suggest(name, cands.iter().map(String::as_str));
        let quoted = |v: &[&str]| {
            v.iter()
                .map(|n| format!("'{n}'"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let tail = if !sugg.is_empty() {
            format!("; did you mean {}?", quoted(&sugg))
        } else if cands.is_empty() {
            "; it has no modifiers".to_owned()
        } else {
            let all: Vec<&str> = cands.iter().map(String::as_str).collect();
            format!("; its modifiers: {}", quoted(&all))
        };
        let msg = if name.is_empty() {
            format!("frap: no modifier given (pass modifier='<name>') for {owner}{tail}")
        } else {
            format!("frap: {owner} has no modifier named '{name}'{tail}")
        };
        cx.report(frap_rna::ReportType::Warning, msg);
    }
}

fn add_poll(c: &Context) -> Result<(), PollFail> {
    shared::active_editable(c, "modifier_add")
}

/// `ED_operator_object_active_editable`.
const ADD_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: add_poll,
};

/// `modifier_add_itemf`: the types the active object supports, in RNA item order.
fn type_items(c: &Context) -> Vec<EnumItemOwned> {
    let PropKind::Enum { items, .. } = &object::MODIFIER_ADD_PROPS[0].kind else {
        return Vec::new();
    };
    let ty = poll::active(c).map(|(_, o)| o.object_type());
    let allowed = registry::addable_types(ty);
    items
        .iter()
        .filter(|i| {
            ModifierType::from_identifier(i.identifier).is_some_and(|t| allowed.contains(&t))
        })
        .map(|i| EnumItemOwned {
            identifier: i.identifier.into(),
            name: i.name.into(),
            description: i.description.into(),
        })
        .collect()
}

#[operator(
    poll = ADD_POLL,
    tier = T1,
    notes = [
        "type items follow the active object's type (none for cameras and lights; NODES only for empties)",
        "persistent_uid is one more than the largest in use (D-47)",
    ],
    dyn_enums = [("type", type_items)]
)]
fn exec(cx: &mut OpCtx, p: object::ModifierAdd) -> OpResult {
    let requested = p.r#type.value.as_str();
    let objects = {
        let c = cx.ctx();
        let items = type_items(&c);
        if !items.iter().any(|i| i.identifier == requested) {
            // Converted without a context: the static superset let it through.
            let list = items
                .iter()
                .map(|i| format!("'{}'", i.identifier))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(OpError::Rna(RnaError::new(
                RnaErrorKind::Enum,
                PyExc::TypeError,
                format!(
                    "Converting py args to operator properties: enum \"{requested}\" not found in ({list})"
                ),
            )));
        }
        shared::edit_objects(&c, p.use_selected_objects.value)
    };
    let ty = ModifierType::from_identifier(requested)
        .ok_or_else(|| OpError::Runtime(format!("unknown modifier type {requested}")))?;
    let mut changed = false;
    for ob in objects {
        let Some(mut o) = cx.main().objects.get(ob).cloned() else {
            continue;
        };
        match edit::add(&mut o, None, ty) {
            Ok(i) => {
                if registry::find(ty).is_none() {
                    // §7.2: added, but frap passes the mesh through.
                    cx.report(
                        ReportType::Warning,
                        stack::pass_through_message(&o.modifiers[i]),
                    );
                }
                // Keep the generation `object_mut` just bumped (`o` carries the old one):
                // the evaluated-object cache keys on it.
                let dst = cx.object_mut(ob)?;
                let generation = dst.header.generation;
                *dst = o;
                dst.header.generation = generation;
                changed = true;
            }
            Err(e) => cx.report(ReportType::Warning, e.to_string()),
        }
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
