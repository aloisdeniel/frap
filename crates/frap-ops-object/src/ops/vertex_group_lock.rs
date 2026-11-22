// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_lock(action='TOGGLE', mask='ALL')`, tier T1
//! (`tests/golden/cases/object/vgroups_m8/copy_lock_move_sort.py`). Port of
//! `vertex_group_lock_exec` / `vgroup_lock_all` / `vgroup_selected_get`
//! (`editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later): changes
//! `VertexGroup.lock_weight` of the active object's groups.
//!
//! * `action`: `LOCK`, `UNLOCK`, `INVERT`, or `TOGGLE` (unlock when one of the masked groups is
//!   locked, else lock).
//! * `mask`: `ALL`, `SELECTED` / `UNSELECTED` (the "selected" groups are those of selected pose
//!   bones in Weight Paint Mode, which frap does not have, so the active group alone), or
//!   `INVERT_UNSELECTED` (the action on every group, then the unselected ones flipped).
//!
//! Poll `vertex_group_poll`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights;

fn lock_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_lock")
}

/// `vertex_group_poll`.
const LOCK_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: lock_poll,
};

/// `vgroup_lock_all(locks, action, mask)` on the lock flags, with the group selection of
/// `vgroup_selected_get`.
fn lock_all(locks: &mut [bool], selected: &[bool], action: &str, mask: &str) {
    let mut action = action;
    let sel = |i: usize| selected.get(i).copied().unwrap_or(false);
    if action == "TOGGLE" {
        action = "LOCK";
        for (i, &l) in locks.iter().enumerate() {
            let skip = match mask {
                "INVERT_UNSELECTED" | "SELECTED" => !sel(i),
                "UNSELECTED" => sel(i),
                _ => false,
            };
            if !skip && l {
                action = "UNLOCK";
                break;
            }
        }
    }
    for (i, l) in locks.iter_mut().enumerate() {
        let skip = match mask {
            "SELECTED" => !sel(i),
            "UNSELECTED" => sel(i),
            _ => false,
        };
        if skip {
            continue;
        }
        match action {
            "LOCK" => *l = true,
            "UNLOCK" => *l = false,
            "INVERT" => *l = !*l,
            _ => {}
        }
        if mask == "INVERT_UNSELECTED" && !sel(i) {
            *l = !*l;
        }
    }
}

#[operator(poll = LOCK_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::VertexGroupLock) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let mut locks = weights::locks(cx, me);
    // `vgroup_selected_get` outside Weight Paint Mode: nothing selected, so the active group.
    let mut selected = vec![false; locks.len()];
    if p.mask.value.identifier() != "ALL" {
        let c = cx.ctx();
        if let Some(a) = c
            .main
            .objects
            .get(ob)
            .and_then(|o| weights::active_group(&c, o))
        {
            selected[a] = true;
        }
    }
    lock_all(&mut locks, &selected, p.action.value.identifier(), p.mask.value.identifier());
    let m = cx.mesh_mut(me)?;
    for (i, l) in locks.into_iter().enumerate() {
        m.set_vertex_group_locked(i, l);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::lock_all;

    #[test]
    fn actions_and_masks() {
        let sel = [false, true, false];
        let mut l = [false, false, false];
        lock_all(&mut l, &sel, "TOGGLE", "ALL");
        assert_eq!(l, [true, true, true]);
        lock_all(&mut l, &sel, "TOGGLE", "ALL");
        assert_eq!(l, [false, false, false]);
        lock_all(&mut l, &sel, "TOGGLE", "SELECTED");
        assert_eq!(l, [false, true, false]);
        lock_all(&mut l, &sel, "INVERT", "INVERT_UNSELECTED");
        assert_eq!(l, [false, false, false]);
        lock_all(&mut l, &sel, "LOCK", "UNSELECTED");
        assert_eq!(l, [true, false, true]);
        lock_all(&mut l, &sel, "TOGGLE", "INVERT_UNSELECTED");
        assert_eq!(l, [false, true, false]);
        lock_all(&mut l, &sel, "UNLOCK", "INVERT_UNSELECTED");
        assert_eq!(l, [true, false, true]);
    }
}
