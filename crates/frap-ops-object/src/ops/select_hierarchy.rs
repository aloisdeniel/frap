// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_hierarchy(direction='PARENT', extend=False)`, tier T0
//! (`tests/golden/cases/object/select_m8/hierarchy*.py`). Ported from
//! blender/scripts/startup/bl_operators/object.py (`SelectHierarchy`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//!
//! - poll: an active object (any mode);
//! - the candidates come from `context.selected_objects` (base order) plus the active object:
//!   `PARENT` takes each one's parent when `visible_get()`, and the active object's parent
//!   becomes active (no visible parent of the active object → no active object at all);
//!   `CHILD` takes the visible children (`Object.children`, `bpy.data.objects` order), sorted
//!   by name, and the first becomes active;
//! - no candidate → `CANCELLED`; otherwise `extend=False` first runs
//!   `object.select_all(action='DESELECT')` (whose poll fails outside Object Mode: `RuntimeError`),
//!   then `select_set(True)` on each candidate (an unselectable one stays unselected).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, ObjectId};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object::{self, select_hierarchy::Direction};

use super::select_grouped::bases::Bases;

/// `return context.object`.
fn has_object(c: &Context) -> Result<(), PollFail> {
    if poll::active(c).is_some() {
        return Ok(());
    }
    let first = c
        .view_layer()
        .bases
        .iter()
        .find_map(|b| c.main.objects.name_of(b.object));
    Err(poll::fail(
        PollReq::ActiveObject,
        match first {
            Some(name) => format!(
                "bpy.context.view_layer.objects.active = bpy.data.objects[{}]",
                poll::py_name(name)
            ),
            None => "add an object first, e.g. bpy.ops.object.empty_add()".into(),
        },
    ))
}

/// `SelectHierarchy.poll`
const HAS_OBJECT: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: has_object,
};

/// The objects to select and the new active object.
fn candidates(
    c: &Context,
    b: &Bases,
    direction: Direction,
) -> (Vec<IdRef<ObjectId>>, Option<IdRef<ObjectId>>) {
    let act = c.active_object();
    let mut from: Vec<IdRef<ObjectId>> = b.selected().into_iter().map(|i| b.list[i].ob).collect();
    if let Some(a) = act
        && !from.contains(&a)
    {
        from.push(a);
    }
    let visible = |o: IdRef<ObjectId>| b.find(o).is_some_and(|i| b.list[i].visible);
    let mut new = Vec::new();
    let mut act_new = None;
    match direction {
        Direction::Parent => {
            for ob in from {
                let Some(par) = c.main.objects.get(ob).and_then(|o| o.parent) else {
                    continue;
                };
                if visible(par) {
                    if act == Some(ob) {
                        act_new = Some(par);
                    }
                    new.push(par);
                }
            }
        }
        Direction::Child => {
            for ob in from {
                // `obj.children`: `bpy.data.objects` order.
                new.extend(
                    c.main
                        .objects
                        .iter()
                        .filter(|(r, o)| o.parent == Some(ob) && visible(*r))
                        .map(|(r, _)| r),
                );
            }
            // `select_new.sort(key=lambda obj_iter: obj_iter.name)` (stable, code points).
            new.sort_by(|a, b| {
                let name = |o: &IdRef<ObjectId>| {
                    c.main
                        .objects
                        .get(*o)
                        .map(|x| x.header.name.clone())
                        .unwrap_or_default()
                };
                name(a).cmp(&name(b))
            });
            act_new = new.first().copied();
        }
    }
    (new, act_new)
}

#[operator(poll = HAS_OBJECT, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::SelectHierarchy) -> OpResult {
    let mut b = Bases::load(cx);
    let (new, act_new, object_mode) = {
        let c = cx.ctx();
        let (new, act_new) = candidates(&c, &b, p.direction.value);
        let object_mode = poll::active(&c).is_none_or(|(_, o)| o.mode == ObjectMode::Object);
        (new, act_new, object_mode)
    };
    if new.is_empty() {
        cx.report(
            ReportType::Warning,
            format!(
                "frap: nothing to select: no visible {} of the selected objects",
                match p.direction.value {
                    Direction::Parent => "parent",
                    Direction::Child => "child",
                }
            ),
        );
        return Ok(OpStatus::Cancelled);
    }
    if !p.extend.value {
        // `bpy.ops.object.select_all(action='DESELECT')`, whose poll needs Object Mode.
        if !object_mode {
            return Err(OpError::Runtime(
                "Operator bpy.ops.object.select_all.poll() failed, context is incorrect".into(),
            ));
        }
        b.deselect_all();
    }
    for ob in new {
        if let Some(i) = b.find(ob) {
            b.select(i, true);
        }
    }
    b.active = act_new;
    b.commit(cx);
    Ok(OpStatus::Finished)
}
