// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_sort(sort_type='NAME')`, tier T1 (`tests/golden/cases/object/
//! vgroups_m8/copy_lock_move_sort.py`, `bone_subsets.py`). Port of `vertex_group_sort_exec`,
//! `vgroup_sort_name`, `vgroup_sort_bone_hierarchy` and `vgroup_do_remap`
//! (`editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later):
//!
//! * `NAME`: the groups in natural, case-insensitive name order (`BLI_strcasecmp_natural`,
//!   stable);
//! * `BONE_HIERARCHY`: the groups named after bones of the deforming armature first, in the
//!   armature's depth-first bone order, the others after them in their current order.
//!
//! Weights and the active index follow their groups.
//!
//! Poll `vertex_group_poll`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_scene::id::Armature;
use frap_spec_ops_object::object;
use smol_str::SmolStr;

use super::vertex_group_add::common;
use crate::support::weights::{self, groups};

fn sort_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_sort")
}

/// `vertex_group_poll`.
const SORT_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: sort_poll,
};

/// `vgroup_sort_bone_hierarchy(ob, bonebase)`: for the bones of `parent` (the roots for
/// `None`), last first, the children are sorted, then the bone's group moves to the head.
fn sort_bone_hierarchy(names: &mut Vec<SmolStr>, arm: &Armature, parent: Option<usize>) {
    for b in arm.children_of(parent).into_iter().rev() {
        let name = arm.bones[b].name.clone();
        let found = names.contains(&name);
        sort_bone_hierarchy(names, arm, Some(b));
        if found && let Some(i) = names.iter().position(|n| *n == name) {
            let g = names.remove(i);
            names.insert(0, g);
        }
    }
}

#[operator(poll = SORT_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::VertexGroupSort) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let mut names = weights::group_names(cx, me);
    match p.sort_type.value.identifier() {
        "BONE_HIERARCHY" => {
            let c = cx.ctx();
            let arm = c
                .main
                .objects
                .get(ob)
                .and_then(|o| weights::deformed_by_armature(&c, o))
                .and_then(|a| c.main.objects.get(a))
                .and_then(|a| a.data)
                .and_then(|d| d.typed())
                .and_then(|d| c.main.armatures.get(d));
            if let Some(arm) = arm {
                sort_bone_hierarchy(&mut names, arm, None);
            }
        }
        _ => names.sort_by(|a, b| groups::strcasecmp_natural(a, b)),
    }
    weights::reorder_groups(cx, ob, me, names)?;
    Ok(OpStatus::Finished)
}
