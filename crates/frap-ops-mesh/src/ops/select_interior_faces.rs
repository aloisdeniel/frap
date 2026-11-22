// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_interior_faces()`, tier T1 (`tests/golden/cases/edit/select_m8/
//! interior_faces.py`). Port of `edbm_faces_select_interior_exec`, `EDBM_select_interior_faces`,
//! `bm_interior_loop_filter_fn`, `bm_interior_edge_is_manifold_except_face_index` and
//! `bm_interior_face_group_calc_cost` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode: the edges with more than two faces split the faces into groups
//! (`BM_mesh_calc_face_groups` stepping over the other edges). Each group gets a cost: the sum
//! over its non-manifold edges of edge length × the angle to the other groups' faces
//! (folded to at most 90°), divided by the group's area. The group of highest cost is selected
//! as interior, the groups it separated merge (`USE_DELAY_FACE_GROUP_COST_CALC`: costs are
//! recalculated when they reach the top of the heap), and so on until no group has a
//! non-manifold edge shared by at least two other faces. Nothing happens on a mesh without
//! non-manifold edges. Always finishes.

use frap_bmesh::ops::beautify_fill::heap::{Heap, HeapNode};
use frap_bmesh::ops::contextual_create::construct::mesh_calc_face_groups;
use frap_bmesh::{BMesh, EdgeId, FaceId, LoopId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;
use super::select_similar::math;

/// Per-call state: the edge tags (`BM_ELEM_TAG` of the non-manifold edges), the edge lengths
/// and the face group index of each face (`BM_elem_index_get`, -1 once selected).
struct State {
    edge_tag: Vec<bool>,
    edge_len: Vec<f32>,
    face_group: Vec<i32>,
}

impl State {
    fn tagged(&self, e: EdgeId) -> bool {
        self.edge_tag.get(e.slot()).copied().unwrap_or(false)
    }
    fn group(&self, f: FaceId) -> i32 {
        self.face_group.get(f.slot()).copied().unwrap_or(-1)
    }
}

/// `bm_interior_edge_is_manifold_except_face_index`.
fn manifold_except(bm: &BMesh, st: &State, e: EdgeId, face_index: i32) -> Option<[LoopId; 2]> {
    let mut pair = Vec::with_capacity(2);
    for l in bm.edge_loops(e) {
        let i = st.group(bm.lp(l).f);
        if i != -1 && i != face_index {
            if pair.len() == 2 {
                return None;
            }
            pair.push(l);
        }
    }
    (pair.len() == 2).then(|| [pair[0], pair[1]])
}

/// `bm_interior_face_group_calc_cost`.
fn calc_cost(bm: &BMesh, st: &State, group: &[(FaceId, f32)]) -> f32 {
    let mut area = 0.0f32;
    let mut cost = 0.0f32;
    let mut found = false;
    for &(f, f_area) in group {
        area += f_area;
        let i = st.group(f);
        for l in bm.face_loops(f) {
            let e = bm.lp(l).e;
            if !st.tagged(e) {
                continue;
            }
            let mut cost_test = 0.0f32;
            let mut cost_count = 0;
            let mut lr = l;
            loop {
                let fo = bm.lp(lr).f;
                let i_other = st.group(fo);
                if i_other != -1 && i_other != i {
                    let mut angle = math::angle_normalized_v3v3(bm.face(f).no, bm.face(fo).no);
                    if angle > 90.0f32.to_radians() {
                        angle = 180.0f32.to_radians() - angle;
                    }
                    cost_test += st.edge_len[e.slot()] * angle;
                    cost_count += 1;
                }
                lr = bm.lp(lr).radial_next;
                if lr == l {
                    break;
                }
            }
            if cost_count >= 2 {
                cost += cost_test;
                found = true;
            }
        }
    }
    if found { cost / area } else { f32::MAX }
}

/// `EDBM_select_interior_faces`: returns whether anything was selected.
pub(crate) fn select_interior_faces(bm: &mut BMesh) -> bool {
    let n_edges = bm.edges.capacity();
    let mut st = State {
        edge_tag: vec![false; n_edges],
        edge_len: vec![-1.0; n_edges],
        face_group: vec![-1; bm.faces.capacity()],
    };
    let mut has_nonmanifold = false;
    for e in bm.edge_ids() {
        if bm.edge_face_count(e) > 2 {
            st.edge_tag[e.slot()] = true;
            st.edge_len[e.slot()] = math::edge_length(bm, e);
            has_nonmanifold = true;
        }
    }
    if !has_nonmanifold {
        return false;
    }
    let tags = st.edge_tag.clone();
    let filter = move |bm: &BMesh, l: LoopId| !tags[bm.lp(l).e.slot()];
    let groups = mesh_calc_face_groups(bm, &filter);
    let mut lists: Vec<Vec<(FaceId, f32)>> = Vec::with_capacity(groups.len());
    for (i, g) in groups.iter().enumerate() {
        let mut list = Vec::with_capacity(g.len());
        for &f in g {
            st.face_group[f.slot()] = i as i32;
            list.push((f, math::face_area(bm, f)));
        }
        lists.push(list);
    }
    let len = lists.len();
    let mut heap: Heap<usize> = Heap::new();
    let mut table: Vec<Option<HeapNode>> = vec![None; len];
    let mut dirty = vec![false; len];
    for i in 0..len {
        let cost = calc_cost(bm, &st, &lists[i]);
        if cost != f32::MAX {
            table[i] = Some(heap.insert(-cost, i));
        }
    }
    let mut recalc: Vec<usize> = Vec::new();
    let mut changed = false;
    loop {
        // `USE_DELAY_FACE_GROUP_COST_CALC`.
        while !heap.is_empty() {
            let i = heap.top_ptr();
            if !dirty[i] {
                break;
            }
            let cost = calc_cost(bm, &st, &lists[i]);
            let node = table[i].expect("heap node of a dirty group");
            if cost != f32::MAX {
                heap.node_value_update(node, -cost);
            } else {
                heap.remove(node);
                table[i] = None;
            }
            dirty[i] = false;
        }
        if heap.is_empty() {
            break;
        }
        let i_min = heap.pop_min();
        table[i_min] = None;
        changed = true;
        let faces = std::mem::take(&mut lists[i_min]);
        for (f, _) in faces {
            bm.face_select_set(f, true);
            st.face_group[f.slot()] = -1;
            for l in bm.face_loops(f) {
                let e = bm.lp(l).e;
                if let Some(pair) = manifold_except(bm, &st, e, i_min as i32) {
                    st.edge_tag[e.slot()] = false;
                    let mut i_a = st.group(bm.lp(pair[0]).f);
                    let mut i_b = st.group(bm.lp(pair[1]).f);
                    if i_a != i_b {
                        if i_a > i_b {
                            std::mem::swap(&mut i_a, &mut i_b);
                        }
                        let (i_a, i_b) = (i_a as usize, i_b as usize);
                        let moved = std::mem::take(&mut lists[i_b]);
                        for &(fb, _) in &moved {
                            st.face_group[fb.slot()] = i_a as i32;
                        }
                        lists[i_a].extend(moved);
                        if let Some(node) = table[i_b].take() {
                            heap.remove(node);
                        }
                        if !dirty[i_a] {
                            recalc.push(i_a);
                            dirty[i_a] = true;
                        }
                    }
                }
                // Mark all connected groups for re-calculation.
                let first = bm.lp(l).radial_next;
                if first != l {
                    let mut lr = first;
                    loop {
                        let i_other = st.group(bm.lp(lr).f);
                        if i_other != -1 && i_other != i_min as i32 {
                            let io = i_other as usize;
                            if table[io].is_some() && !dirty[io] {
                                dirty[io] = true;
                            }
                        }
                        lr = bm.lp(lr).radial_next;
                        if lr == l {
                            break;
                        }
                    }
                }
            }
        }
        for &i in &recalc {
            if let Some(node) = table[i]
                && dirty[i]
            {
                let cost = calc_cost(bm, &st, &lists[i]);
                if cost != f32::MAX {
                    heap.node_value_update(node, -cost);
                } else {
                    heap.remove(node);
                    table[i] = None;
                }
            }
            dirty[i] = false;
        }
        recalc.clear();
    }
    changed
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::SelectInteriorFaces) -> OpResult {
    for (_, me) in common::edit_meshes(cx) {
        let has_nonmanifold = common::read(cx, me, |bm| {
            bm.edge_ids().into_iter().any(|e| bm.edge_face_count(e) > 2)
        })
        .unwrap_or(false);
        if !has_nonmanifold {
            continue;
        }
        select_interior_faces(cx.bmesh_mut(me)?);
    }
    Ok(OpStatus::Finished)
}
