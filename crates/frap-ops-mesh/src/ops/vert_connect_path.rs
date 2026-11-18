// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.vert_connect_path()` ("J"), tier T1
//! (`tests/golden/cases/edit/create/vert_connect_path.py`). Port of
//! `edbm_vert_connect_path_exec`, `bm_vert_connect_select_history`,
//! `bm_vert_connect_pair` and `bm_vert_connect_select_history_edge_to_vert_path`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! The selected vertices are connected **in selection order** (the selection history, e.g.
//! `frap.select_elements(type='VERT', indices=[…])`): across faces with
//! `connect_vert_pair` when every vertex has faces, else by new edges; a path whose edges all
//! exist is closed from the last vertex to the first. Two selected vertices behave like
//! `mesh.vert_connect`. An edge-only history is turned into a vertex path first. Errors
//! (`Invalid selection order`, `Could not connect mixed selection types`, `Could not connect a
//! face selection`, `Could not connect vertices`) are raised when no mesh succeeded.

use frap_bmesh::ops::connect_vert_pair::{ConnectVertPairIn, connect_vert_pair};
use frap_bmesh::ops::contextual_create::{bli, construct};
use frap_bmesh::types::CreateFlag;
use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, LoopId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;
use super::vert_connect::connect_vert_pair_mesh;

/// `bm_vert_is_select_history_open`: both ends of the history are vertices with exactly one
/// selected edge.
fn select_history_open(bm: &BMesh) -> bool {
    let (Some(&BMElem::Vert(a)), Some(&BMElem::Vert(b))) =
        (bm.select_history.first(), bm.select_history.last())
    else {
        return false;
    };
    let count = |v: VertId| {
        bm.vert_edges(v)
            .into_iter()
            .filter(|&e| bm.edge(e).flag.contains(ElemFlag::SELECT))
            .count()
    };
    count(a) == 1 && count(b) == 1
}

/// `bm_vert_connect_pair`.
fn vert_connect_pair(bm: &mut BMesh, v_a: VertId, v_b: VertId, skip_normals: bool) -> bool {
    let totedge_orig = bm.edges_num();
    if !skip_normals {
        bm.vert_normal_update(v_a);
        bm.vert_normal_update(v_b);
    }
    if let Ok(out) = connect_vert_pair(
        bm,
        &ConnectVertPairIn {
            verts: vec![v_a, v_b],
            ..Default::default()
        },
    ) {
        let el: Vec<BMElem> = out.edges.iter().map(|&e| BMElem::Edge(e)).collect();
        edbm::select_enable(bm, &el, true);
    }
    bm.edges_num() != totedge_orig
}

/// `BM_vert_is_wire` (or no edge at all).
fn vert_wire_or_loose(bm: &BMesh, v: VertId) -> bool {
    bm.vert(v).e.is_none() || construct::vert_is_wire(bm, v)
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (bm_vert_connect_select_history).
fn connect_select_history(bm: &mut BMesh) -> bool {
    let hist = bm.select_history.clone();
    if hist.len() < 2 || bm.selected_counts().0 <= 2 {
        return false;
    }
    let mut tot = 0usize;
    let mut has_wire = false;
    for &ese in &hist {
        let BMElem::Vert(v) = ese else {
            break;
        };
        if !has_wire && vert_wire_or_loose(bm, v) {
            has_wire = true;
        }
        tot += 1;
    }
    let vert_at = |i: usize| match hist[i] {
        BMElem::Vert(v) => v,
        _ => unreachable!("history checked to be vertices"),
    };
    if !has_wire {
        let mut changed = false;
        if tot == bm.selected_counts().0 {
            let is_multi_cut = bm.selected_counts().0 > 2;
            let mut orig: rustc_hash::FxHashMap<VertId, [f32; 3]> = Default::default();
            if is_multi_cut {
                for i in 0..hist.len() {
                    let v = vert_at(i);
                    bm.vert_normal_update(v);
                    orig.entry(v).or_insert(bm.vert(v).no);
                }
            }
            for i in 1..hist.len() {
                let (v_last, v_curr) = (vert_at(i - 1), vert_at(i));
                if bm.edge_exists(v_last, v_curr).is_some() {
                    continue;
                }
                if is_multi_cut {
                    bm.vert_mut(v_last).no = orig[&v_last];
                    bm.vert_mut(v_curr).no = orig[&v_curr];
                }
                changed |= vert_connect_pair(bm, v_last, v_curr, is_multi_cut);
            }
            if changed {
                return true;
            }
        }
        if !changed && select_history_open(bm) {
            let (a, b) = (vert_at(0), vert_at(hist.len() - 1));
            if vert_connect_pair(bm, a, b, false) {
                return true;
            }
        }
        false
    } else {
        let mut changed = false;
        for i in 1..hist.len() {
            let (vp, v) = (vert_at(i - 1), vert_at(i));
            if bm.edge_exists(vp, v).is_none() {
                let e = bm.edge_create(vp, v, None, CreateFlag::empty());
                bm.edge_select_set(e, true);
                changed = true;
            }
        }
        if !changed && select_history_open(bm) {
            let e = bm.edge_create(vert_at(0), vert_at(hist.len() - 1), None, CreateFlag::empty());
            bm.edge_select_set(e, true);
        }
        true
    }
}

/// `BM_edge_pair_share_face_by_len(e_a, e_b, allow_adjacent = true)`: the smallest face using
/// both edges, with their loops.
fn edge_pair_share_face_by_len(bm: &BMesh, e_a: EdgeId, e_b: EdgeId) -> Option<(LoopId, LoopId)> {
    let mut best: Option<(u32, LoopId, LoopId)> = None;
    for la in bm.edge_loops(e_a) {
        let f = bm.lp(la).f;
        if let Some(lb) = bm.face_edge_share_loop(f, e_b) {
            let len = bm.face(f).len;
            if best.is_none_or(|b| len < b.0) {
                best = Some((len, la, lb));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (bm_vert_connect_select_history_edge_to_vert_path).
fn edge_to_vert_path(bm: &BMesh) -> Option<Vec<BMElem>> {
    let mut edges = Vec::new();
    for &ese in &bm.select_history {
        match ese {
            BMElem::Edge(e) => edges.push(e),
            _ => return None,
        }
    }
    if bm.selected_counts().1 != edges.len() {
        return None;
    }
    let mut out: std::collections::VecDeque<BMElem> = Default::default();
    let mut side = false;
    for (i, &e_curr) in edges.iter().enumerate() {
        if i > 0 {
            let e_prev = edges[i - 1];
            if let Some((l_curr, l_prev)) = edge_pair_share_face_by_len(bm, e_curr, e_prev) {
                let ec = bm.edge(e_curr).v;
                let ep = bm.edge(e_prev).v;
                if (ec[0] != bm.lp(l_curr).v) == (ep[0] != bm.lp(l_prev).v) {
                    side = !side;
                }
            } else {
                let ec = bm.edge(e_curr).v;
                let ep = bm.edge(e_prev).v;
                let co = |v: VertId| bm.vert(v).co;
                if bli::is_quad_flip_v3(co(ec[0]), co(ec[1]), co(ep[1]), co(ep[0])) {
                    side = !side;
                }
            }
        }
        let ev = bm.edge(e_curr).v;
        let v = ev[side as usize];
        if out.back() != Some(&BMElem::Vert(v)) {
            out.push_back(BMElem::Vert(v));
        }
        let v = ev[!side as usize];
        if out.front() != Some(&BMElem::Vert(v)) {
            out.push_front(BMElem::Vert(v));
        }
    }
    Some(out.into_iter().collect())
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::VertConnectPath) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let mut failed_order = 0usize;
    let mut failed_connect = 0usize;
    let mut mixed = false;
    let mut face = false;
    for &(_, me) in &meshes {
        let Some((tv, hist)) = edbm::with_bm(cx, me, |bm| {
            (bm.selected_counts().0, bm.select_history.clone())
        }) else {
            continue;
        };
        if tv == 0 {
            continue;
        }
        if tv == 2 {
            if connect_vert_pair_mesh(cx, me)? == 0 {
                failed_connect += 1;
            }
            continue;
        }
        let has_v = hist.iter().any(|e| matches!(e, BMElem::Vert(_)));
        let has_e = hist.iter().any(|e| matches!(e, BMElem::Edge(_)));
        let has_f = hist.iter().any(|e| matches!(e, BMElem::Face(_)));
        if (has_v as u8 + has_e as u8 + has_f as u8) > 1 {
            mixed = true;
            failed_order += 1;
            continue;
        }
        if has_f {
            face = true;
            failed_order += 1;
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let mut selected_orig: Option<Vec<BMElem>> = None;
        if matches!(bm.select_history.first(), Some(BMElem::Edge(_)))
            && let Some(path) = edge_to_vert_path(bm)
        {
            selected_orig = Some(std::mem::replace(&mut bm.select_history, path));
        }
        if connect_select_history(bm) {
            bm.select_flush_mode();
        } else {
            failed_order += 1;
        }
        if let Some(orig) = selected_orig {
            bm.select_history = orig;
        }
    }
    if failed_order == meshes.len() {
        let msg = if mixed {
            "Could not connect mixed selection types"
        } else if face {
            "Could not connect a face selection"
        } else {
            "Invalid selection order"
        };
        return Err(OpError::Runtime(msg.into()));
    }
    if failed_connect == meshes.len() {
        return Err(OpError::Runtime("Could not connect vertices".into()));
    }
    Ok(OpStatus::Finished)
}
