// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.posemode_toggle()`, tier T0 (WP 6.2). Port of `posemode_exec`
//! (blender/source/blender/editors/object/object_edit.cc) and `mode_compat_set`
//! (blender/source/blender/editors/object/object_modes.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! - Active object in Pose Mode: it and every other armature in Pose Mode go back to Object
//!   Mode.
//! - Otherwise the current mode is left first (Edit Mode through its handler, which writes the
//!   edit bones back), then the active armature and the other selected, editable armatures in
//!   Object Mode enter Pose Mode.
//! - A non-armature active object only leaves its mode (Blender answers `PASS_THROUGH`, which
//!   Python treats as success: frap answers `FINISHED`).
//! - A hidden active object (no base in the view layer, `hide_viewport`) is `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_scene::ObjectMode;
use frap_spec_ops_object::object;

fn status(r: OpStatus) -> Result<(), OpError> {
    match r {
        OpStatus::Finished => Ok(()),
        OpStatus::Cancelled => Err(OpError::Runtime(
            "Unable to execute 'Toggle Pose Mode', error changing modes".into(),
        )),
    }
}

#[operator(poll = poll::ACTIVE_EDITABLE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::PosemodeToggle) -> OpResult {
    let (active, ty, mode, visible) = {
        let c = cx.ctx();
        let Some((r, o)) = poll::active(&c) else {
            return Ok(OpStatus::Cancelled);
        };
        let visible = c.visible_objects().contains(&r);
        (r, o.object_type(), o.mode, visible)
    };
    if !visible {
        return Ok(OpStatus::Cancelled);
    }
    if mode == ObjectMode::Pose {
        let others: Vec<IdRef<ObjectId>> = {
            let c = cx.ctx();
            c.view_layer()
                .bases
                .iter()
                .map(|b| b.object)
                .filter(|&r| {
                    c.main
                        .objects
                        .get(r)
                        .is_some_and(|o| o.mode == ObjectMode::Pose && o.object_type() == "ARMATURE")
                })
                .collect()
        };
        let mut objects = vec![active];
        objects.extend(others.into_iter().filter(|&r| r != active));
        let h = helpers::mode_handler(ObjectMode::Pose, "ARMATURE")?;
        status((h.exit)(cx, &objects)?)?;
        return Ok(OpStatus::Finished);
    }
    if mode != ObjectMode::Object {
        // `mode_compat_set`: the current mode's toggle leaves it.
        helpers::mode_set_object(cx)?;
    }
    if ty != "ARMATURE" {
        return Ok(OpStatus::Finished);
    }
    let objects: Vec<IdRef<ObjectId>> = {
        let c = cx.ctx();
        let mut v = vec![active];
        for r in c.selected_objects() {
            if r == active {
                continue;
            }
            if c.main.objects.get(r).is_some_and(|o| {
                o.object_type() == "ARMATURE"
                    && o.mode == ObjectMode::Object
                    && o.header.library.is_none()
            }) {
                v.push(r);
            }
        }
        v
    };
    let h = helpers::mode_handler(ObjectMode::Pose, "ARMATURE")?;
    status((h.enter)(cx, &objects)?)?;
    Ok(OpStatus::Finished)
}
