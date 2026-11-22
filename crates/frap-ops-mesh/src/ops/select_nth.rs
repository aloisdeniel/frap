// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_nth(skip=1, nth=1, offset=0)`, tier T1 (`tests/golden/cases/edit/
//! select_m8/nth*.py`). Port of `edbm_select_nth_exec`, `edbm_deselect_nth`,
//! `deselect_nth_active`, `walker_deselect_nth`, the cyclic-chain walks
//! (`bm_*_form_cyclic_chain`, `walker_deselect_nth_*_chain`, `bm_step_*_in_chain`) and
//! `bm_edge_is_select_isolated` (`editors/mesh/editmesh_select.cc`), with
//! `WM_operator_properties_checker_interval_from_op/_test`, Blender 5.2.4, GPL-2.0-or-later;
//! the walkers are in `walk.rs`.
//!
//! Per mesh in Edit Mode with a selection: after flushing, the start element is the active
//! element when selected (the last selection-history entry), else the first selected vertex
//! (vertex mode), edge (edge mode) or the active/any selected face (face mode). From it the
//! selected elements of its kind are walked breadth first (vertices over edges, edges over
//! vertices, or across faces when no other selected edge touches the start edge, faces across
//! edges) and an element at depth `d` stays selected when `(offset + d) % (skip + nth) >=
//! skip`, else it is deselected; a selection forming a single closed chain (a ring of
//! vertices, edges or faces) is walked along the ring instead. The selection is then flushed
//! from that kind. Without a start element on any mesh the operator is `CANCELLED` with
//! Blender's error `Mesh object(s) have no active vertex/edge/face`.

mod walk;

use frap_bmesh::{BMElem, BMesh, EdgeId, FaceId, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;
use super::shortest_path_select::Interval;

fn sel(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    common::selected(bm, e)
}

// ----------------------------------------------------------------------- cyclic chains

/// `bm_step_to_next_selected_vert_in_chain`.
fn next_vert_in_chain(bm: &BMesh, v_curr: VertId, v_prev: Option<VertId>) -> Option<VertId> {
    let expected = if v_prev.is_some() { 1 } else { 2 };
    let mut count = 0;
    let mut next = None;
    for e in bm.vert_edges(v_curr) {
        let o = bm.edge_other_vert(e, v_curr);
        if Some(o) == v_prev || !sel(bm, o) {
            continue;
        }
        count += 1;
        if count > expected {
            return None;
        }
        next = Some(o);
    }
    if count == expected { next } else { None }
}

/// `bm_step_over_vert_to_next_selected_edge_in_chain`.
fn next_edge_in_chain(bm: &BMesh, e_curr: EdgeId, v: VertId) -> Option<EdgeId> {
    let mut count = 0;
    let mut next = None;
    for e in bm.vert_edges(v) {
        if e == e_curr || !sel(bm, e) {
            continue;
        }
        count += 1;
        if count > 1 {
            return None;
        }
        next = Some(e);
    }
    if count == 1 { next } else { None }
}

/// `bm_step_over_shared_edge_to_next_selected_face_in_chain`.
fn next_face_in_chain(bm: &BMesh, f_curr: FaceId, f_prev: Option<FaceId>) -> Option<FaceId> {
    let expected = if f_prev.is_some() { 1 } else { 2 };
    let mut count = 0;
    let mut next = None;
    for l in bm.face_loops(f_curr) {
        for fo in bm.edge_faces(bm.lp(l).e) {
            if fo == f_curr || Some(fo) == f_prev || !sel(bm, fo) {
                continue;
            }
            count += 1;
            if count > expected {
                return None;
            }
            next = Some(fo);
        }
    }
    if count == expected { next } else { None }
}

/// `bm_verts_form_cyclic_chain`.
fn verts_form_cyclic_chain(bm: &BMesh, v_start: VertId) -> bool {
    let mut v_prev = None;
    let mut v_curr = v_start;
    loop {
        let mut n = 0;
        for e in bm.vert_edges(v_curr) {
            if sel(bm, bm.edge_other_vert(e, v_curr)) {
                n += 1;
                if n > 2 {
                    return false;
                }
            }
        }
        if n != 2 {
            return false;
        }
        let Some(v_next) = next_vert_in_chain(bm, v_curr, v_prev) else {
            return false;
        };
        v_prev = Some(v_curr);
        v_curr = v_next;
        if v_curr == v_start {
            return true;
        }
    }
}

/// `bm_edges_form_cyclic_chain`.
fn edges_form_cyclic_chain(bm: &BMesh, e_start: EdgeId) -> bool {
    let mut e_curr = e_start;
    let mut v_through = bm.edge(e_start).v[0];
    loop {
        let Some(e_next) = next_edge_in_chain(bm, e_curr, v_through) else {
            return false;
        };
        v_through = bm.edge_other_vert(e_next, v_through);
        e_curr = e_next;
        if e_curr == e_start {
            return true;
        }
    }
}

/// `bm_faces_form_cyclic_chain`.
fn faces_form_cyclic_chain(bm: &BMesh, f_start: FaceId) -> bool {
    let mut f_prev = None;
    let mut f_curr = f_start;
    loop {
        let mut n = 0;
        for l in bm.face_loops(f_curr) {
            for fo in bm.edge_faces(bm.lp(l).e) {
                if fo != f_curr && sel(bm, fo) {
                    n += 1;
                }
            }
        }
        if n != 2 {
            return false;
        }
        let Some(f_next) = next_face_in_chain(bm, f_curr, f_prev) else {
            return false;
        };
        f_prev = Some(f_curr);
        f_curr = f_next;
        if f_curr == f_start {
            return true;
        }
    }
}

/// `walker_deselect_nth_vertex_chain`.
fn deselect_vertex_chain(bm: &mut BMesh, iv: &Interval, v_start: VertId) {
    let mut visited = vec![false; bm.verts.capacity()];
    let mut v_prev = None;
    let mut v_curr = Some(v_start);
    let mut index = 0;
    while let Some(v) = v_curr {
        if visited[v.slot()] {
            break;
        }
        visited[v.slot()] = true;
        if !iv.test(index) {
            bm.vert_select_set(v, false);
        }
        let next = next_vert_in_chain(bm, v, v_prev);
        if next.is_none() || next == Some(v_start) {
            break;
        }
        v_prev = Some(v);
        v_curr = next;
        index += 1;
    }
}

/// `walker_deselect_nth_edge_chain`.
fn deselect_edge_chain(bm: &mut BMesh, iv: &Interval, e_start: EdgeId) {
    let mut visited = vec![false; bm.edges.capacity()];
    let mut e_curr = e_start;
    let mut v_through = bm.edge(e_start).v[0];
    let mut index = 0;
    while !visited[e_curr.slot()] {
        visited[e_curr.slot()] = true;
        if !iv.test(index) {
            bm.edge_select_set(e_curr, false);
        }
        let next = next_edge_in_chain(bm, e_curr, v_through);
        let Some(e_next) = next.filter(|&e| e != e_start) else {
            break;
        };
        v_through = bm.edge_other_vert(e_next, v_through);
        e_curr = e_next;
        index += 1;
    }
}

/// `walker_deselect_nth_face_chain`.
fn deselect_face_chain(bm: &mut BMesh, iv: &Interval, f_start: FaceId) {
    let mut visited = vec![false; bm.faces.capacity()];
    let mut f_prev = None;
    let mut f_curr = Some(f_start);
    let mut index = 0;
    while let Some(f) = f_curr {
        if visited[f.slot()] {
            break;
        }
        visited[f.slot()] = true;
        let next = next_face_in_chain(bm, f, f_prev);
        if !iv.test(index) {
            bm.face_select_set(f, false);
        }
        if next.is_none() || next == Some(f_start) {
            break;
        }
        f_prev = Some(f);
        f_curr = next;
        index += 1;
    }
}

// ------------------------------------------------------------------------------ the walk

/// `bm_edge_is_select_isolated`.
fn edge_is_select_isolated(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge(e)
        .v
        .into_iter()
        .all(|v| bm.vert_edges(v).into_iter().all(|o| o == e || !sel(bm, o)))
}

/// `walker_deselect_nth`.
fn walker_deselect_nth(bm: &mut BMesh, iv: &Interval, act: BMElem) {
    match act {
        BMElem::Vert(v) if verts_form_cyclic_chain(bm, v) => {
            deselect_vertex_chain(bm, iv, v);
            bm.select_flush_mode_ex(SelectMode::VERT, false);
            return;
        }
        BMElem::Edge(e) if edges_form_cyclic_chain(bm, e) => {
            deselect_edge_chain(bm, iv, e);
            bm.select_flush_mode_ex(SelectMode::EDGE, false);
            return;
        }
        BMElem::Face(f) if faces_form_cyclic_chain(bm, f) => {
            deselect_face_chain(bm, iv, f);
            bm.select_flush_mode_ex(SelectMode::FACE, false);
            return;
        }
        _ => {}
    }
    // The walker masks: the selection before the walk (`BMO_ELE_TAG`).
    let (flushtype, walked): (SelectMode, Vec<(BMElem, i32)>) = match act {
        BMElem::Vert(v) => {
            let tag: Vec<bool> = slots(bm.verts.capacity(), bm.vert_ids(), |x| sel(bm, x));
            let mask = |x: VertId| tag[x.slot()];
            let w = walk::connected_vertex(bm, v, &mask);
            (
                SelectMode::VERT,
                w.into_iter().map(|(x, d)| (BMElem::Vert(x), d)).collect(),
            )
        }
        BMElem::Edge(e) => {
            let tag: Vec<bool> = slots(bm.edges.capacity(), bm.edge_ids(), |x| sel(bm, x));
            let mask = |x: EdgeId| tag[x.slot()];
            let w = if edge_is_select_isolated(bm, e) {
                walk::face_shell(bm, e, &mask)
            } else {
                walk::vert_shell(bm, e, &mask)
            };
            (
                SelectMode::EDGE,
                w.into_iter().map(|(x, d)| (BMElem::Edge(x), d)).collect(),
            )
        }
        BMElem::Face(f) => {
            let tag: Vec<bool> = slots(bm.faces.capacity(), bm.face_ids(), |x| sel(bm, x));
            let mask = |x: FaceId| tag[x.slot()];
            let w = walk::island(bm, f, &mask);
            (
                SelectMode::FACE,
                w.into_iter().map(|(x, d)| (BMElem::Face(x), d)).collect(),
            )
        }
    };
    for (ele, depth) in walked {
        if !iv.test(depth) {
            bm.select_set(ele, false);
        }
    }
    bm.select_flush_mode_ex(flushtype, false);
}

/// A per-slot table of `f` over `ids`.
fn slots<T: Copy + Slot>(cap: usize, ids: Vec<T>, f: impl Fn(T) -> bool) -> Vec<bool> {
    let mut v = vec![false; cap];
    for x in ids {
        v[x.slot_index()] = f(x);
    }
    v
}

trait Slot {
    fn slot_index(self) -> usize;
}
impl Slot for VertId {
    fn slot_index(self) -> usize {
        self.slot()
    }
}
impl Slot for EdgeId {
    fn slot_index(self) -> usize {
        self.slot()
    }
}
impl Slot for FaceId {
    fn slot_index(self) -> usize {
        self.slot()
    }
}

/// `BM_mesh_active_face_get(bm, true, false)`: the active face, else the newest visible face
/// of the selection history, else the first selected face.
fn active_face_sloppy(bm: &BMesh) -> Option<FaceId> {
    if let Some(f) = bm.act_face
        && bm.elem_alive(BMElem::Face(f))
    {
        return Some(f);
    }
    for e in bm.select_history.iter().rev() {
        if let BMElem::Face(f) = *e
            && !common::hidden(bm, f)
        {
            return Some(f);
        }
    }
    bm.face_ids().into_iter().find(|&f| sel(bm, f))
}

/// `deselect_nth_active` (after `EDBM_selectmode_flush`).
fn deselect_nth_active(bm: &BMesh) -> Option<BMElem> {
    if let Some(&ele) = bm.select_history.last()
        && sel(bm, ele)
    {
        return Some(ele);
    }
    let mode = bm.select_mode;
    if mode.contains(SelectMode::VERT) {
        return bm
            .vert_ids()
            .into_iter()
            .find(|&v| sel(bm, v))
            .map(BMElem::Vert);
    }
    if mode.contains(SelectMode::EDGE) {
        return bm
            .edge_ids()
            .into_iter()
            .find(|&e| sel(bm, e))
            .map(BMElem::Edge);
    }
    if mode.contains(SelectMode::FACE) {
        return active_face_sloppy(bm)
            .filter(|&f| sel(bm, f))
            .map(BMElem::Face);
    }
    None
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectNth) -> OpResult {
    let iv = Interval::new(p.nth.value, p.skip.value, p.offset.value);
    let mut found_active_elt = false;
    for (_, me) in common::edit_meshes(cx) {
        if !common::read(cx, me, common::any_selected).unwrap_or(false) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        common::selectmode_flush(bm);
        if let Some(act) = deselect_nth_active(bm) {
            walker_deselect_nth(bm, &iv, act);
            found_active_elt = true;
        }
    }
    if !found_active_elt {
        return Err(OpError::Runtime(
            "Mesh object(s) have no active vertex/edge/face".into(),
        ));
    }
    Ok(OpStatus::Finished)
}
