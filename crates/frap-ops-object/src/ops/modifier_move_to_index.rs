// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_move_to_index(modifier='', index=0, use_selected_objects=False)`,
//! tier T1. Port of `modifier_move_to_index_exec` (`editors/object/object_modifier.cc`, Blender
//! 5.2.4, GPL-2.0-or-later) over `frap_modifiers::stack::edit::move_to_index` with
//! `allow_partial = true` (a move stops before the first refused step).
//!
//! Verified on 5.2.4: `index=0` moves `Bevel` before `Subdivision`; `index=5` on a stack of two
//! is `CANCELLED` with the WARNING `Cannot move modifier beyond the end of the stack`.

use frap_modifiers::stack::edit;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::modifier_add::shared;

fn move_poll(c: &Context) -> Result<(), PollFail> {
    shared::edit_modifier_poll(c, "modifier_move_to_index", true)
}

/// `edit_modifier_poll`.
const MOVE_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: move_poll,
};

#[operator(poll = MOVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ModifierMoveToIndex) -> OpResult {
    let objects = shared::edit_objects(&cx.ctx(), p.use_selected_objects.value);
    let mut changed = false;
    for ob in objects {
        let Some(mut mods) = cx.main().objects.get(ob).map(|o| o.modifiers.clone()) else {
            continue;
        };
        let Some(i) = mods.iter().position(|m| m.name == p.modifier.value) else {
            continue;
        };
        match edit::move_to_index(&mut mods, i, i64::from(p.index.value), true) {
            Ok(()) => {
                if cx
                    .main()
                    .objects
                    .get(ob)
                    .is_some_and(|o| o.modifiers != mods)
                {
                    cx.object_mut(ob)?.modifiers = mods;
                }
                changed = true;
            }
            Err(Some(msg)) => cx.report(ReportType::Warning, msg),
            Err(None) => {}
        }
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
