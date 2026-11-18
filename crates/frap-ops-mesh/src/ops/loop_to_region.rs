// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.loop_to_region(select_bigger=False)`, tier T1 (`tests/golden/cases/edit/
//! select/loop_to_region*.py`). Port of `edbm_loop_to_region_exec`, `loop_find_regions`,
//! `loop_find_region` and `verg_radial` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode with selected edges, the selected edges split the faces into regions;
//! for each selected edge the smaller (`select_bigger`: the bigger) region on its sides is kept,
//! choosing between the "smaller" and "bigger" passes so that the total face count is minimal
//! (maximal with `select_bigger`). The selection is replaced by the faces of the kept regions
//! (hidden faces stay unselected) and flushed; the selection history is cleared. Selected
//! edges are visited from those with the most faces to those with the fewest (Blender's
//! `qsort`; frap sorts stably, which only matters between edges with equal face counts).

use frap_bmesh::{BMesh, EdgeId, FaceId, LoopId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use rustc_hash::FxHashSet;

use super::select_more::common;

/// `loop_find_region`: the faces reachable from `l.f` without crossing a selected edge or a
/// face tagged by an earlier region; `visit` is shared by every call of one pass.
fn loop_find_region(
    bm: &BMesh,
    l: LoopId,
    face_tag: &FxHashSet<FaceId>,
    visit: &mut FxHashSet<FaceId>,
) -> Vec<FaceId> {
    let mut stack = vec![bm.lp(l).f];
    let mut region = Vec::new();
    visit.insert(bm.lp(l).f);
    while let Some(f) = stack.pop() {
        region.push(f);
        for l1 in bm.face_loops(f) {
            let e = bm.lp(l1).e;
            if common::selected(bm, e) {
                continue;
            }
            for l2 in bm.edge_loops(e) {
                let f2 = bm.lp(l2).f;
                // Avoids finding the same region twice.
                if face_tag.contains(&f2) {
                    continue;
                }
                if visit.insert(f2) {
                    stack.push(f2);
                }
            }
        }
    }
    region
}

/// `loop_find_regions`: tags (in `face_tag`) the faces of the chosen region of every selected
/// edge and returns their total count.
fn loop_find_regions(bm: &BMesh, face_tag: &mut FxHashSet<FaceId>, selbigger: bool) -> usize {
    let mut visit: FxHashSet<FaceId> = FxHashSet::default();
    let mut edges: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| common::selected(bm, e))
        .collect();
    let mut edge_tag: FxHashSet<EdgeId> = edges.iter().copied().collect();
    // Sort edges by radial cycle length, longest first.
    edges.sort_by_key(|&e| std::cmp::Reverse(bm.edge_face_count(e)));
    let mut count = 0;
    for e in edges {
        if !edge_tag.contains(&e) {
            continue;
        }
        let mut region: Option<Vec<FaceId>> = None;
        let mut tot = 0;
        for l in bm.edge_loops(e) {
            if visit.contains(&bm.lp(l).f) {
                continue;
            }
            let found = loop_find_region(bm, l, face_tag, &mut visit);
            let c = found.len();
            let better = if selbigger { c >= tot } else { c < tot };
            if region.is_none() || better {
                tot = c;
                region = Some(found);
            }
        }
        if let Some(region) = region {
            for &f in &region {
                face_tag.insert(f);
                for e2 in bm.face_edges(f) {
                    edge_tag.remove(&e2);
                }
            }
            count += tot;
        }
    }
    count
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::LoopToRegion) -> OpResult {
    let select_bigger = p.select_bigger.value;
    for (_, me) in common::edit_meshes(cx) {
        let any_edge = common::read(cx, me, |bm| bm.selected_counts().1 != 0).unwrap_or(false);
        if !any_edge {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        // Find the set of regions with the smallest number of total faces.
        let mut face_tag: FxHashSet<FaceId> = FxHashSet::default();
        let a = loop_find_regions(bm, &mut face_tag, select_bigger);
        let b = loop_find_regions(bm, &mut face_tag, !select_bigger);
        face_tag.clear();
        let pass = if (a <= b) != select_bigger {
            select_bigger
        } else {
            !select_bigger
        };
        loop_find_regions(bm, &mut face_tag, pass);
        // Unlike most operators, always deselect all.
        common::deselect_all(bm);
        for f in bm.face_ids() {
            if common::hidden(bm, f) {
                continue;
            }
            if face_tag.contains(&f) {
                bm.face_select_set(f, true);
            }
        }
        common::selectmode_flush(bm);
    }
    Ok(OpStatus::Finished)
}
