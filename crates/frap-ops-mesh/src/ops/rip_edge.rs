// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.rip_edge(location=(0, 0, 0), direction=(0, 0, 0))`, tier T1
//! (`tests/golden/cases/edit/tools_m8/rip_edge.py`). Port of `edbm_rip_edge_exec`
//! (`editors/mesh/editmesh_rip_edge.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Extends every selected vertex (of every mesh in Edit Mode) along one of its edges: the edge
//! is split at the vertex (`BM_edge_split`, factor 0), the new vertex takes the selection and
//! the old one is deselected. The edge is the only selected visible edge of the vertex
//! ("tricky extend"; a vertex with two or more selected edges is left alone), else the visible
//! edge whose direction is closest to the direction from the vertex to its projection on the
//! world-space ray `location` + t·`direction` (the mouse ray `invoke` fills in). The default
//! zero direction measures towards `location` itself. The selection history is cleared and
//! the selection flushed on every changed mesh. Always `FINISHED`. frap keeps the UV sync
//! selection only within one operator (D-111), so the UV-select transfer of Blender's
//! `uv_select_sync_valid` branch never applies.

use frap_bmesh::{ElemFlag, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh;

pub(crate) mod math;

use math::{
    add3, angle_normalized_v3v3, closest_to_line_v3, normalize3, normalize_v3, sub3,
    transform_point,
};

#[operator(poll = poll::EDITMESH_VIEW3D, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::RipEdge) -> OpResult {
    let ray_loc = p.location.value;
    let ray_dir = normalize_v3(p.direction.value).0;

    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let object_to_world = world_matrix_in(cx.main(), ob).to_cols_array_2d();
        if super::split::edbm::read(cx, me, |bm| bm.selected_counts().0) == Some(0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let mut changed = false;
        // Clear tags.
        let mut tagged = vec![false; bm.verts.capacity()];
        // Operate on selected verts (the iteration also visits the vertices created below,
        // which are tagged).
        let mut slot = 0usize;
        while slot < bm.verts.capacity() {
            let v = VertId(slot as u32);
            slot += 1;
            let Some(vert) = bm.verts.get(v.0) else {
                continue;
            };
            if !vert.flag.contains(ElemFlag::SELECT) || tagged.get(v.slot()) == Some(&true) {
                continue;
            }
            let edges = bm.vert_edges(v);
            // First check whether the edge to split can be chosen from the selection alone.
            let mut tot_sel = 0;
            let mut e_best = None;
            for &e in &edges {
                let f = bm.edge(e).flag;
                if !f.contains(ElemFlag::HIDDEN) && f.contains(ElemFlag::SELECT) {
                    e_best = Some(e);
                    tot_sel += 1;
                }
            }
            if tot_sel != 1 {
                e_best = None;
            }
            if e_best.is_none() && tot_sel == 0 {
                let mut angle_best = f32::MAX;
                let v_world = transform_point(&object_to_world, bm.vert(v).co);
                for &e in &edges {
                    if bm.edge(e).flag.contains(ElemFlag::HIDDEN) {
                        continue;
                    }
                    let v_other = bm.edge_other_vert(e, v);
                    let (projected, _) =
                        closest_to_line_v3(v_world, ray_loc, add3(ray_loc, ray_dir));
                    let v_ray_dir = normalize3(sub3(projected, v_world));
                    let v_other_world = transform_point(&object_to_world, bm.vert(v_other).co);
                    let v_edge_dir = normalize3(sub3(v_other_world, v_world));
                    let angle_test = angle_normalized_v3v3(v_ray_dir, v_edge_dir);
                    if angle_test < angle_best {
                        angle_best = angle_test;
                        e_best = Some(e);
                    }
                }
            }
            let Some(e_best) = e_best else {
                continue;
            };
            let e_select = bm.edge(e_best).flag.contains(ElemFlag::SELECT);
            let (v_new, e_new) = bm.edge_split(e_best, v, 0.0);
            bm.vert_select_set(v, false);
            bm.edge_select_set(e_new, false);
            bm.vert_select_set(v_new, true);
            if e_select {
                bm.edge_select_set(e_best, true);
            }
            // Prevent further splitting.
            if tagged.len() <= v_new.slot() {
                tagged.resize(v_new.slot() + 1, false);
            }
            tagged[v_new.slot()] = true;
            changed = true;
        }
        if changed {
            bm.select_history_clear();
            bm.select_flush_mode();
        }
    }
    Ok(OpStatus::Finished)
}
