// SPDX-License-Identifier: GPL-3.0-or-later
//! The Object Mode handler of `MODE_HANDLERS` (ARCHITECTURE §8, M1), for every object type.
//!
//! Object Mode holds no state of its own: leaving it does nothing (the next mode's handler
//! sets `Object.mode` and builds its edit data), and entering it only makes sure
//! `Object.mode` reads `OBJECT` after the previous mode's handler wrote its data back (an edit
//! handler's `exit` writes the edit BMesh to the mesh and resets the mode itself, because
//! `helpers::add_*` leave Edit Mode through it alone).

use frap_op::prelude::*;
use frap_op::{MODE_HANDLERS, ModeHandler};
use frap_rna::{IdRef, ObjectId};
use frap_scene::ObjectMode;

/// Sets `Object.mode = 'OBJECT'` on `objects` that are in another mode (touching only those).
fn enter(cx: &mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult {
    for &ob in objects {
        if cx
            .main()
            .objects
            .get(ob)
            .is_some_and(|o| o.mode != ObjectMode::Object)
        {
            cx.object_mut(ob)?.mode = ObjectMode::Object;
            cx.main_mut().1.context_changed = true;
        }
    }
    Ok(OpStatus::Finished)
}

/// Leaving Object Mode needs nothing.
fn exit(_: &mut OpCtx, _: &[IdRef<ObjectId>]) -> OpResult {
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(MODE_HANDLERS)]
static OBJECT: ModeHandler = ModeHandler {
    mode: ObjectMode::Object,
    object_type: "",
    enter,
    exit,
};
