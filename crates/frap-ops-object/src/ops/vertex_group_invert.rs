// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_invert(group_select_mode='', auto_assign=True,
//! auto_remove=True)`, tier T1 (`tests/golden/cases/object/vgroups_m8/invert_levels_quantize.py`).
//! Port of `vertex_group_invert_exec` / `vgroup_invert_subset` (`editors/object/
//! object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later): on the active object, every weight of
//! the subset (the active group unless asked otherwise; groups visited last first) becomes
//! `1 - w`, for every vertex (the selected ones in Edit Mode). `auto_assign` first adds the
//! vertices missing from a group (so they end at 1), `auto_remove` then drops the weights that
//! became 0. Locked groups are inverted too, as in Blender.
//!
//! Poll `vertex_group_poll`.

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Topology, Weights};

fn invert_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_invert")
}

/// `vertex_group_poll`.
const INVERT_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: invert_poll,
};

/// `rna_vertex_group_with_single_itemf`.
fn subset_items(c: &Context) -> Vec<EnumItemOwned> {
    weights::subset_items(c, true)
}

#[operator(poll = INVERT_POLL, tier = T1, dyn_enums = [("group_select_mode", subset_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupInvert) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let mode = weights::subset_mode(&p.group_select_mode.value, true);
    let (validmap, _) = weights::subset(&cx.ctx(), ob, mode);
    let (auto_assign, auto_remove) = (p.auto_assign.value, p.auto_remove.value);
    let use_vert_sel = common::use_vert_sel(cx, ob);
    let (use_mirror, use_topology) = weights::symmetry(cx, me);
    let Some(mut w) = Weights::load(cx, me) else {
        return Ok(OpStatus::Finished);
    };
    let mut in_array: Vec<bool> = (0..w.len()).map(|i| w.in_array(i, use_vert_sel)).collect();
    for (i, dv) in w.dv.iter_mut().enumerate() {
        if !in_array[i] {
            continue;
        }
        for j in (0..validmap.len()).rev() {
            if !validmap[j] {
                continue;
            }
            let k = if auto_assign {
                deform::ensure(dv, j as u32);
                deform::find(dv, j as u32)
            } else {
                deform::find(dv, j as u32)
            };
            if let Some(k) = k {
                dv.weights[k].1 = (1.0 - dv.weights[k].1).clamp(0.0, 1.0);
            }
        }
    }
    if use_mirror && use_vert_sel {
        let names = weights::group_names(cx, me);
        let topo = Topology::load(cx, me).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
        let mirr = weights::mirror_map(&w, &topo, use_topology);
        let flip_map = weights::groups::flip_map(&names, &[], true, false);
        weights::mirror_sync(&mut w.dv, &mut in_array, &mirr, &validmap, &flip_map);
    }
    if auto_remove {
        for (i, dv) in w.dv.iter_mut().enumerate() {
            if in_array[i] {
                deform::remove_zero(dv, &validmap, 0.0, false);
            }
        }
    }
    w.store(cx)?;
    Ok(OpStatus::Finished)
}
