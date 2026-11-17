// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_remove_all()`, tier T0 (`tests/golden/cases/object/slots/*.py`).
//! Port of `material_slot_remove_all_exec` (`editors/render/render_shading.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4: every slot of the **selected** editable mesh objects
//! is removed (through `BKE_object_material_slot_remove`, so objects sharing a mesh lose them
//! too; face indices end at 0 and the `material_index` attribute stays); INFO `Removed N
//! materials`, `CANCELLED` when nothing was removed (no selected mesh with slots); poll as
//! `material_slot_remove` (the active object needs a slot).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::id::slots;
use frap_spec_ops_object::object;

use super::material_slot_add::common;

fn remove_poll(c: &Context) -> Result<(), PollFail> {
    common::remove_poll(c, "object.material_slot_remove_all")
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
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotRemoveAll) -> OpResult {
    let objects = common::shading_objects(&cx.ctx());
    let mut removed = 0usize;
    let (main, log) = cx.main_mut();
    for ob in objects {
        loop {
            let tot = main.objects.get(ob).map_or(0, |o| slots::slot_count(main, o));
            if tot == 0 || !common::slot_remove_at(main, log, ob, tot - 1) {
                break;
            }
            removed += 1;
        }
    }
    if removed == 0 {
        cx.report(
            ReportType::Warning,
            "frap: nothing removed: no selected mesh object has material slots",
        );
        return Ok(OpStatus::Cancelled);
    }
    cx.report(ReportType::Info, format!("Removed {removed} materials"));
    Ok(OpStatus::Finished)
}
