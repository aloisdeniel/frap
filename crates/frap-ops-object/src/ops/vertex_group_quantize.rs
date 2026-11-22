// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_quantize(group_select_mode='', steps=4)`, tier T1
//! (`tests/golden/cases/object/vgroups_m8/invert_levels_quantize.py`). Port of
//! `vertex_group_quantize_exec` / `vgroup_quantize_subset` (`editors/object/
//! object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later): on the active object, every weight of
//! the subset (the active group unless asked otherwise) is rounded to the nearest multiple of
//! `1 / steps` (`floorf(w * steps + 0.5) / steps`), for every vertex (the selected ones in
//! Edit Mode). Locked groups change too, as in Blender.
//!
//! Poll `vertex_group_poll`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Topology, Weights};

fn quantize_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_quantize")
}

/// `vertex_group_poll`.
const QUANTIZE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: quantize_poll,
};

/// `rna_vertex_group_with_single_itemf`.
fn subset_items(c: &Context) -> Vec<EnumItemOwned> {
    weights::subset_items(c, true)
}

/// One weight of `vgroup_quantize_subset`.
fn quantize(w: f32, steps: f32) -> f32 {
    (libm::floorf((w * steps) + 0.5) / steps).clamp(0.0, 1.0)
}

#[operator(poll = QUANTIZE_POLL, tier = T1, dyn_enums = [("group_select_mode", subset_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupQuantize) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let mode = weights::subset_mode(&p.group_select_mode.value, true);
    let (validmap, _) = weights::subset(&cx.ctx(), ob, mode);
    let steps = p.steps.value as f32;
    let use_vert_sel = common::use_vert_sel(cx, ob);
    let (use_mirror, use_topology) = weights::symmetry(cx, me);
    let Some(mut w) = Weights::load(cx, me) else {
        return Ok(OpStatus::Finished);
    };
    let mut in_array: Vec<bool> = (0..w.len()).map(|i| w.in_array(i, use_vert_sel)).collect();
    if use_mirror && use_vert_sel {
        let topo = Topology::load(cx, me).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
        let mirr = weights::mirror_map(&w, &topo, use_topology);
        weights::mirror_assign(&mut in_array, &mirr);
    }
    for (i, dv) in w.dv.iter_mut().enumerate() {
        if !in_array[i] {
            continue;
        }
        for e in dv.weights.iter_mut() {
            if validmap.get(e.0 as usize).copied().unwrap_or(false) {
                e.1 = quantize(e.1, steps);
            }
        }
    }
    w.store(cx)?;
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::quantize;

    #[test]
    fn rounds_to_steps() {
        assert_eq!(quantize(0.3, 2.0), 0.5);
        assert_eq!(quantize(0.2, 2.0), 0.0);
        assert_eq!(quantize(0.25, 2.0), 0.5);
        assert_eq!(quantize(0.6, 4.0), 0.5);
    }
}
