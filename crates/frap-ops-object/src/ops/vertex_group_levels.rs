// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_levels(group_select_mode='', offset=0.0, gain=1.0)`, tier T1
//! (`tests/golden/cases/object/vgroups_m8/invert_levels_quantize.py`). Port of
//! `vertex_group_levels_exec` / `vgroup_levels_subset` (`editors/object/object_vgroup.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): on the active object, every existing weight of the subset
//! (the active group unless asked otherwise) becomes `gain * (w + offset)` clamped to 0..1, for
//! every vertex (the selected ones in Edit Mode). Locked groups change too, as in Blender.
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

fn levels_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_levels")
}

/// `vertex_group_poll`.
const LEVELS_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: levels_poll,
};

/// `rna_vertex_group_with_single_itemf`.
fn subset_items(c: &Context) -> Vec<EnumItemOwned> {
    weights::subset_items(c, true)
}

#[operator(poll = LEVELS_POLL, tier = T1, dyn_enums = [("group_select_mode", subset_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupLevels) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let mode = weights::subset_mode(&p.group_select_mode.value, true);
    let (validmap, _) = weights::subset(&cx.ctx(), ob, mode);
    let (offset, gain) = (p.offset.value, p.gain.value);
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
            if validmap[j]
                && let Some(k) = deform::find(dv, j as u32)
            {
                dv.weights[k].1 = (gain * (dv.weights[k].1 + offset)).clamp(0.0, 1.0);
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
    w.store(cx)?;
    Ok(OpStatus::Finished)
}
