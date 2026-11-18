// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.editmode_toggle()`, tier T1 (`tests/golden/cases/edit/prereq/
//! editmode_toggle*.py`). Port of `editmode_toggle_exec` and `editmode_toggle_poll`
//! (`editors/object/object_edit.cc`, Blender 5.2.4, GPL-2.0-or-later), through the `EDIT` mode
//! handler of the active object's type (`modes/edit_mesh.rs` for meshes; other types answer
//! `OP_NOT_IMPLEMENTED` until their handler lands):
//! - poll: an active object with data, the data local, not hidden in the viewport unless it is
//!   in Edit Mode, of a type that has an Edit Mode;
//! - in Object Mode: the active object enters Edit Mode, and when it did, every other selected
//!   object of its type too (a second user of an edited mesh is skipped);
//! - in Edit Mode: the active object leaves it, and when it did, every other object of its type
//!   in the view layer too.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, ObjectId};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;

/// `OB_TYPE_SUPPORT_EDITMODE`.
fn supports_editmode(object_type: &str) -> bool {
    matches!(
        object_type,
        "MESH"
            | "CURVE"
            | "SURFACE"
            | "FONT"
            | "META"
            | "LATTICE"
            | "ARMATURE"
            | "CURVES"
            | "POINTCLOUD"
            | "GREASEPENCIL"
    )
}

/// `editmode_toggle_poll`.
fn toggle_poll(c: &Context) -> Result<(), PollFail> {
    let Some((_, ob)) = poll::active(c) else {
        return Err(poll::fail(
            PollReq::ActiveObject,
            "bpy.context.view_layer.objects.active = bpy.data.objects['<name>']",
        ));
    };
    let data_local = ob
        .data
        .and_then(|d| c.main.header(d))
        .is_some_and(|h| h.library.is_none());
    if !data_local {
        return Err(poll::fail(
            PollReq::ActiveEditable,
            "make the active object's data local",
        ));
    }
    if ob.hide_viewport && ob.mode != ObjectMode::Edit {
        return Err(poll::fail(
            PollReq::ActiveObject,
            format!(
                "bpy.data.objects[{}].hide_viewport = False",
                poll::py_name(&ob.header.name)
            ),
        ));
    }
    if !supports_editmode(ob.object_type()) {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make a mesh (or another type with an Edit Mode) the active object",
        ));
    }
    Ok(())
}

/// `editmode_toggle_poll`
const TOGGLE_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: toggle_poll,
};

fn in_edit_mode(cx: &OpCtx, ob: IdRef<ObjectId>) -> bool {
    cx.main()
        .objects
        .get(ob)
        .is_some_and(|o| o.mode == ObjectMode::Edit)
}

#[operator(
    poll = TOGGLE_POLL,
    tier = T1,
    support = Partial("meshes only: other object types answer OP_NOT_IMPLEMENTED until their Edit Mode handler lands (armatures: M6)"),
    notes = ["entering also enters every selected object of the active object's type (multi-object editing)"]
)]
fn exec(cx: &mut OpCtx, _p: object::EditmodeToggle) -> OpResult {
    let (active, ty) = {
        let c = cx.ctx();
        let (r, ob) = poll::active(&c)
            .ok_or_else(|| OpError::Runtime("Context missing active object".into()))?;
        (r, ob.object_type())
    };
    let handler = helpers::mode_handler(ObjectMode::Edit, ty)?;
    if !in_edit_mode(cx, active) {
        if !matches!((handler.enter)(cx, &[active])?, OpStatus::Finished)
            || !in_edit_mode(cx, active)
        {
            return Ok(OpStatus::Finished);
        }
        let others: Vec<IdRef<ObjectId>> = {
            let c = cx.ctx();
            c.selected_objects()
                .into_iter()
                .filter(|&r| {
                    r != active && c.main.objects.get(r).is_some_and(|o| o.object_type() == ty)
                })
                .collect()
        };
        // `editmode_enter_ex` per object: one that cannot enter (linked, no data) does not
        // stop the others (the handler treats the first object of a slice as the active one).
        for ob in others {
            (handler.enter)(cx, &[ob])?;
        }
    } else {
        (handler.exit)(cx, &[active])?;
        if !in_edit_mode(cx, active) {
            let others: Vec<IdRef<ObjectId>> = {
                let c = cx.ctx();
                c.view_layer()
                    .bases
                    .iter()
                    .map(|b| b.object)
                    .filter(|&r| {
                        r != active
                            && c.main.objects.get(r).is_some_and(|o| {
                                o.object_type() == ty && o.mode == ObjectMode::Edit
                            })
                    })
                    .collect()
            };
            if !others.is_empty() {
                (handler.exit)(cx, &others)?;
            }
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_mode_types_match_blender() {
        assert!(supports_editmode("MESH"));
        assert!(supports_editmode("ARMATURE"));
        assert!(supports_editmode("LATTICE"));
        assert!(!supports_editmode("CAMERA"));
        assert!(!supports_editmode("LIGHT"));
        assert!(!supports_editmode("EMPTY"));
    }
}
