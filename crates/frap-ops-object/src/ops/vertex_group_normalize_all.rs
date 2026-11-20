// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_normalize_all(group_select_mode='', lock_active=True)`, tier
//! T0. Port of `vertex_group_normalize_all_exec`, `normalize_vertex_group_target`,
//! `vgroup_normalize_all` and `BKE_defvert_normalize_ex` (`editors/object/object_vgroup.cc`,
//! `blenkernel/intern/deform.cc`, Blender 5.2.4, GPL-2.0-or-later): for every vertex (the
//! selected ones in Edit Mode) the weights of the subset are scaled to sum to 1, the active
//! group's kept with `lock_active`. As in Blender, the subset is not the one asked for:
//! `group_select_mode` is overwritten with `ALL`, or `BONE_DEFORM` when an armature deforms the
//! object and one of its deform bones has a group. A vertex with a single weight gets 1.
//!
//! Locked groups (`lock_weight`) keep their weights; `lock_active` locks the active group too.
//! Every group locked: `RuntimeError: Error: All groups are locked` (nothing changes).

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn normalize_all_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_normalize_all")
}

/// `vertex_group_poll`.
const NORMALIZE_ALL_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: normalize_all_poll,
};

/// `rna_vertex_group_select_itemf`.
fn subset_items(c: &Context) -> Vec<EnumItemOwned> {
    common::subset_items(c, false)
}

#[operator(poll = NORMALIZE_ALL_POLL, tier = T0, dyn_enums = [("group_select_mode", subset_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupNormalizeAll) -> OpResult {
    let c = cx.ctx();
    let Some((ob, o)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(me) = common::mesh_of(&c, o) else {
        return Ok(OpStatus::Cancelled);
    };
    // `normalize_vertex_group_target`.
    let target = if common::validmap_deform(&c, o).iter().any(|x| *x) {
        "BONE_DEFORM"
    } else {
        "ALL"
    };
    let (validmap, subset_count) = common::subset(&c, o, target);
    let vgroup_tot = validmap.len();
    let def_nr = usize::try_from(o.vertex_groups_active_index)
        .ok()
        .filter(|&a| a < vgroup_tot);
    if subset_count == 0 {
        return Err(OpError::Runtime("No vertex groups to operate on".into()));
    }
    // `BKE_object_defgroup_lock_flags_get` (`None` when no group is locked), then the active
    // group with `lock_active`.
    let mut lock_flags: Vec<bool> = common::mesh_of(&c, o)
        .and_then(|me| c.main.meshes.get(me))
        .and_then(|m| m.vertex_group_lock_flags())
        .map(|mut l| {
            l.resize(vgroup_tot, false);
            l
        })
        .unwrap_or_default();
    if p.lock_active.value {
        if lock_flags.is_empty() {
            lock_flags = vec![false; vgroup_tot];
        }
        if let Some(a) = def_nr {
            lock_flags[a] = true;
        }
    }
    if !lock_flags.is_empty() && !lock_flags.contains(&false) {
        return Err(OpError::Runtime("All groups are locked".into()));
    }
    let sel = common::use_vert_sel(cx, ob);
    common::for_each_dvert(cx, me, sel, |dv| {
        deform::normalize_ex(dv, &validmap, &lock_flags, &[]);
    })?;
    Ok(OpStatus::Finished)
}
