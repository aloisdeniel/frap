// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_remove()`, tier T0 (`tests/golden/cases/object/slots/*.py`).
//! Port of `material_slot_remove_exec` and `BKE_object_material_slot_remove`
//! (`editors/render/render_shading.cc`, `blenkernel/intern/material.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//! - removes the **active** slot of the active object (also when it is hidden with `hide_set`)
//!   from its data, so every object sharing the mesh loses that slot; their object-linked
//!   entries above it shift down and their active slot is clamped to the new count;
//! - face `material_index` values above the removed slot are decremented (0 stays 0); the
//!   attribute is kept;
//! - poll: the material poll of `material_slot_add`, plus at least one slot (`failed, context
//!   is incorrect`) and not in Edit Mode (`Unable to remove material slot in edit mode`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::material_slot_add::common;

fn remove_poll(c: &Context) -> Result<(), PollFail> {
    common::remove_poll(c, "object.material_slot_remove")
}

/// `object_materials_supported_poll` + a slot + Object Mode.
const REMOVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeObject,
    ],
    check: remove_poll,
};

#[operator(poll = REMOVE_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotRemove) -> OpResult {
    let Some(ob) = cx.ctx().object() else {
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    common::slot_remove(main, log, ob);
    Ok(OpStatus::Finished)
}
