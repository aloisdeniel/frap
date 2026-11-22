// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_smooth(group_select_mode='', factor=0.5, repeat=1,
//! expand=0.0)`, tier T1 (`tests/golden/cases/object/vgroups_m8/smooth.py`). Port of
//! `vertex_group_smooth_exec` and `vgroup_smooth_subset` (`editors/object/object_vgroup.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode (each mesh once), for every group of the subset (the active
//! group unless asked otherwise) that is not locked, `repeat` Jacobi passes move the weight of
//! every selected vertex with a visible neighbour towards the mean of its visible neighbours'
//! weights (`w' = (1 - factor) w + factor mean`, unassigned vertices count as 0). `expand > 0`
//! favours larger neighbours (a smaller neighbour counts as `expand w + (1 - expand) w_n` with
//! weight `1 - expand`), `expand < 0` smaller ones. Afterwards every vertex of the mesh is
//! written back: positive weights are assigned, zero ones removed (also on unselected
//! vertices).
//!
//! No mesh with vertex groups: WARNING `No meshes with vertex groups found` (`CANCELLED`).
//!
//! Poll `vertex_group_mesh_vert_poll`: a mesh in Edit Mode (Weight Paint Mode does not exist in
//! frap).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Topology, Weights};
use frap_bmesh::shape::deform;

fn smooth_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_group_smooth")
}

/// `vertex_group_mesh_vert_poll`.
const SMOOTH_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: smooth_poll,
};

/// `rna_vertex_group_with_single_itemf`.
fn subset_items(c: &Context) -> Vec<EnumItemOwned> {
    weights::subset_items(c, true)
}

/// The smoothing parameters.
struct Smooth {
    fac: f32,
    repeat: i32,
    fac_expand: f32,
}

impl Smooth {
    /// `vgroup_smooth_subset` on one mesh: the snapshot `w` (every vertex) and its topology;
    /// `use_select` (Edit Mode) restricts the written vertices to the selected ones and the
    /// read neighbours to the visible ones.
    fn run(&self, w: &mut Weights, topo: &Topology, validmap: &[bool], use_select: bool) {
        let n = w.len();
        let ifac = 1.0 - self.fac;
        let expand_sign = if self.fac_expand > 0.0 {
            1
        } else if self.fac_expand < 0.0 {
            -1
        } else {
            0
        };
        let expand = self.fac_expand.abs();
        let iexpand = 1.0 - expand;
        let readable = |v: usize| !use_select || !w.hide[v];
        let writable = |v: usize| !use_select || w.select[v];
        // The vertices that will be smoothed: writable, with a readable neighbour.
        let verts_used: Vec<usize> = (0..n)
            .filter(|&i| {
                writable(i)
                    && topo.vert_edges[i]
                        .iter()
                        .any(|&e| readable(topo.other(e, i)))
            })
            .collect();
        for (def_nr, _) in validmap.iter().enumerate().filter(|(_, v)| **v) {
            let g = def_nr as u32;
            let mut prev: Vec<f32> = w.dv.iter().map(|d| deform::weight(d, g)).collect();
            let mut curr = prev.clone();
            for _ in 0..self.repeat {
                for &i in &verts_used {
                    let mut weight_tot = 0.0f32;
                    let mut weight = 0.0f32;
                    for &e in &topo.vert_edges[i] {
                        let o = topo.other(e, i);
                        if !readable(o) {
                            continue;
                        }
                        let mut weight_other = prev[o];
                        let mut tot_factor = 1.0f32;
                        if (expand_sign == 1 && weight_other < prev[i])
                            || (expand_sign == -1 && weight_other > prev[i])
                        {
                            weight_other = (prev[i] * expand) + (weight_other * iexpand);
                            tot_factor = iexpand;
                        }
                        weight += tot_factor * weight_other;
                        weight_tot += tot_factor;
                    }
                    if weight_tot != 0.0 {
                        weight /= weight_tot;
                        weight = (prev[i] * ifac) + (weight * self.fac);
                        curr[i] = weight.clamp(0.0, 1.0);
                    }
                }
                std::mem::swap(&mut curr, &mut prev);
            }
            // `vgroup_parray_from_weight_array(…, remove_zero = true)` on every vertex.
            for (dv, &wt) in w.dv.iter_mut().zip(&prev) {
                if wt > 0.0 {
                    *deform::ensure(dv, g) = wt;
                } else if let Some(k) = deform::find(dv, g) {
                    deform::remove_at(dv, k);
                }
            }
        }
    }
}

#[operator(poll = SMOOTH_POLL, tier = T1, dyn_enums = [("group_select_mode", subset_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupSmooth) -> OpResult {
    let mode = weights::subset_mode(&p.group_select_mode.value, true);
    let smooth = Smooth {
        fac: p.factor.value,
        repeat: p.repeat.value,
        fac_expand: p.expand.value,
    };
    let objects = common::objects_for_wpaint(&cx.ctx());
    let mut has_vgroup_multi = false;
    for ob in objects {
        let (me, mut validmap, mut subset_count) = {
            let c = cx.ctx();
            let Some(me) = c
                .main
                .objects
                .get(ob)
                .and_then(|o| weights::mesh_of(&c, o))
            else {
                continue;
            };
            let (v, k) = weights::subset(&c, ob, mode);
            (me, v, k)
        };
        if validmap.is_empty() {
            continue;
        }
        if let Some(locked) = weights::lock_flags(cx, me) {
            for (v, l) in validmap.iter_mut().zip(locked) {
                if *v && l {
                    *v = false;
                    subset_count -= 1;
                }
            }
        }
        has_vgroup_multi = true;
        if subset_count == 0 {
            continue;
        }
        let use_select = common::use_vert_sel(cx, ob);
        let (use_mirror, use_topology) = weights::symmetry(cx, me);
        let Some(mut w) = Weights::load(cx, me) else {
            continue;
        };
        let topo = Topology::load(cx, me).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
        smooth.run(&mut w, &topo, &validmap, use_select);
        if use_mirror {
            let names = weights::group_names(cx, me);
            let mirr = weights::mirror_map(&w, &topo, use_topology);
            let flip_map = weights::groups::flip_map(&names, &[], true, false);
            let mut in_array: Vec<bool> = (0..w.len()).map(|i| w.in_array(i, true)).collect();
            weights::mirror_sync(&mut w.dv, &mut in_array, &mirr, &validmap, &flip_map);
        }
        w.store(cx)?;
    }
    if !has_vgroup_multi {
        cx.report(ReportType::Warning, "No meshes with vertex groups found");
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
