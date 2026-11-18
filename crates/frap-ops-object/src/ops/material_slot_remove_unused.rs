// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_remove_unused()`, tier T0
//! (`tests/golden/cases/object/slots/*.py`). Port of `material_slot_remove_unused_exec`
//! (`editors/render/render_shading.cc`, Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4:
//! - works on the **selected** editable mesh objects (`object_array_for_shading_edit_mode_
//!   disabled`: an unselected active object is left alone), each in base order;
//! - removes every slot no face uses (without a `material_index` attribute only slot 0 is
//!   used; out-of-range indices use nothing), through `BKE_object_material_slot_remove`, so
//!   objects sharing the mesh lose them too; each object's active slot follows its slot;
//! - INFO `Removed N slots`; nothing removed → `CANCELLED`;
//! - poll as `material_slot_remove` (the active object needs a slot).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::id::slots;
use frap_spec_ops_object::object;

use super::material_slot_add::common;

fn remove_poll(c: &Context) -> Result<(), PollFail> {
    common::remove_poll(c, "object.material_slot_remove_unused")
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
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotRemoveUnused) -> OpResult {
    let objects = common::shading_objects(&cx.ctx());
    let mut removed = 0usize;
    let (main, log) = cx.main_mut();
    for ob in objects {
        let Some(o) = main.objects.get(ob) else {
            continue;
        };
        let Some(me) = common::mesh_of(main, ob) else {
            continue;
        };
        let mut act = common::actcol(o, slots::slot_count(main, o));
        let mut slot = 1usize;
        loop {
            let tot = main
                .objects
                .get(ob)
                .map_or(0, |o| slots::slot_count(main, o));
            if slot > tot {
                break;
            }
            let used = main
                .meshes
                .get(me)
                .is_some_and(|m| common::slot_used(m, slot - 1));
            if used {
                slot += 1;
                continue;
            }
            common::slot_remove_at(main, log, ob, slot - 1);
            if act >= slot {
                act -= 1;
            }
            removed += 1;
        }
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.active_material_index = act.saturating_sub(1) as i32;
        }
    }
    if removed == 0 {
        cx.report(
            ReportType::Warning,
            "frap: nothing removed: every material slot of the selected objects is used",
        );
        return Ok(OpStatus::Cancelled);
    }
    cx.report(ReportType::Info, format!("Removed {removed} slots"));
    Ok(OpStatus::Finished)
}
