// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.edge_face_add()` ("F", Make Edge/Face), tier T1
//! (`tests/golden/cases/edit/create/edge_face_add_*.py`). Port of `edbm_add_edge_face_exec`
//! and its `USE_FACE_CREATE_SEL_EXTEND` helpers (`editors/mesh/editmesh_tools.cc`, Blender
//! 5.2.4, GPL-2.0-or-later) over `bmesh.ops.contextual_create`.
//!
//! Per mesh in Edit Mode with a selection:
//! - a single selected vertex with two free wire (or boundary) edges, or a selected edge
//!   with one at each end, first extends the selection along them so repeated presses walk
//!   around a hole;
//! - `contextual_create` runs on the selection with the active material slot and the
//!   majority smooth flag of the selected edges' faces;
//! - nothing created: the mesh is left alone; otherwise the new faces and edges are
//!   selected (and unhidden), or, after a selection extension that made one face, only the
//!   next edge/vertex along the boundary is selected and becomes active.
//!
//! The operator is `CANCELLED` when no mesh changed. `contextual_create` dissolves selected
//! faces through WP 2.7's `dissolve_faces`.

pub(crate) mod edbm;

use frap_bmesh::ops::contextual_create::{ContextualCreateIn, construct, contextual_create};
use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

/// `edbm_add_edge_face_exec__vert_edge_lookup`: up to `max` visible edges of `v` (other than
/// `e_used`) passing `test`.
fn vert_edge_lookup(
    bm: &BMesh,
    v: VertId,
    e_used: Option<EdgeId>,
    max: usize,
    test: EdgeTest,
) -> Vec<EdgeId> {
    let mut out = Vec::new();
    for e in bm.vert_edges(v) {
        if !bm.edge(e).flag.contains(ElemFlag::HIDDEN) && Some(e) != e_used && test(bm, e) {
            out.push(e);
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

/// An edge predicate of `edbm_add_edge_face_exec__vert_edge_lookup`.
type EdgeTest = fn(&BMesh, EdgeId) -> bool;

fn is_wire(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge_is_wire(e)
}

fn is_boundary(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge_is_boundary(e)
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (edbm_add_edge_face_exec__tricky_extend_sel).
fn tricky_extend_sel(bm: &mut BMesh) -> Option<BMElem> {
    let (tv, te, tf) = bm.selected_counts();
    if tv == 1 && te == 0 && tf == 0 {
        let v = edbm::sel_verts(bm).into_iter().next()?;
        let mut pair = None;
        for test in [is_wire as EdgeTest, is_boundary] {
            let ed = vert_edge_lookup(bm, v, None, 3, test);
            if ed.len() == 2 && !construct::edge_share_face_check(bm, ed[0], ed[1]) {
                pair = Some((ed[0], ed[1]));
                break;
            }
        }
        let (a, b) = pair?;
        let e_other = bm.edge_exists(bm.edge_other_vert(a, v), bm.edge_other_vert(b, v));
        bm.edge_select_set(a, true);
        bm.edge_select_set(b, true);
        if let Some(eo) = e_other {
            bm.edge_select_set(eo, true);
        }
        return Some(BMElem::Vert(v));
    }
    if tv == 2 && te == 1 && tf == 0 {
        let e = edbm::sel_edges(bm).into_iter().next()?;
        let [v1, v2] = bm.edge(e).v;
        let combos: [(EdgeTest, EdgeTest); 4] = [
            (is_wire, is_wire),
            (is_wire, is_boundary),
            (is_boundary, is_wire),
            (is_boundary, is_boundary),
        ];
        let mut found = None;
        for (t1, t2) in combos {
            let p1 = vert_edge_lookup(bm, v1, Some(e), 2, t1);
            let p2 = vert_edge_lookup(bm, v2, Some(e), 2, t2);
            if p1.len() == 1
                && p2.len() == 1
                && !construct::edge_share_face_check(bm, e, p1[0])
                && !construct::edge_share_face_check(bm, e, p2[0])
            {
                found = Some((p1[0], p2[0]));
                break;
            }
        }
        let (a, b) = found?;
        let v1_other = bm.edge_other_vert(a, v1);
        let v2_other = bm.edge_other_vert(b, v2);
        let e_other = if v1_other != v2_other {
            bm.edge_exists(v1_other, v2_other)
        } else {
            None
        };
        bm.edge_select_set(a, true);
        bm.edge_select_set(b, true);
        if let Some(eo) = e_other {
            bm.edge_select_set(eo, true);
        }
        return Some(BMElem::Edge(e));
    }
    None
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (edbm_add_edge_face_exec__tricky_finalize_sel).
fn tricky_finalize_sel(bm: &mut BMesh, ele_desel: BMElem, f: FaceId) {
    bm.select_history_clear();
    bm.face_mut(f).flag.remove(ElemFlag::HIDDEN);
    bm.face_select_set(f, false);
    match ele_desel {
        BMElem::Vert(v) => {
            let l = bm.face_vert_share_loop(f, v).expect("face uses the vertex");
            bm.vert_select_set(v, false);
            let e = bm.lp(bm.lp(l).next).e;
            bm.edge_select_set(e, true);
            bm.select_history_store(BMElem::Edge(e));
        }
        BMElem::Edge(e) => {
            let l = bm.face_edge_share_loop(f, e).expect("face uses the edge");
            bm.edge_select_set(e, false);
            let nn = bm.lp(bm.lp(l).next).next;
            if bm.face(f).len == 4 {
                let e_active = bm.lp(nn).e;
                bm.edge_mut(e_active).flag.remove(ElemFlag::HIDDEN);
                bm.edge_select_set(e_active, true);
                bm.select_history_store(BMElem::Edge(e_active));
            } else {
                let v_active = bm.lp(nn).v;
                bm.vert_mut(v_active).flag.remove(ElemFlag::HIDDEN);
                bm.vert_select_set(v_active, true);
                bm.select_history_store(BMElem::Vert(v_active));
            }
        }
        BMElem::Face(_) => {}
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::EdgeFaceAdd) -> OpResult {
    run(cx, p)
}

fn run(cx: &mut OpCtx, _p: mesh::EdgeFaceAdd) -> OpResult {
    let mut changed_multi = false;
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let any = edbm::with_bm(cx, me, |bm| {
            let (v, e, f) = bm.selected_counts();
            v + e + f > 0
        });
        if any != Some(true) {
            continue;
        }
        let mat_nr = edbm::mat_nr(cx, ob);
        let bm = cx.bmesh_mut(me)?;
        let use_smooth = edbm::smooth_get(bm);
        let totedge_orig = bm.edges_num();
        let totface_orig = bm.faces_num();
        let ele_desel = tricky_extend_sel(bm);
        let geom = edbm::sel_geom(bm);
        let r = contextual_create(
            bm,
            &ContextualCreateIn {
                geom,
                mat_nr,
                use_smooth,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        if totedge_orig == bm.edges_num() && totface_orig == bm.faces_num() {
            // Nothing created: Blender keeps the (possibly extended) selection.
            continue;
        }
        if let Some(desel) = ele_desel
            && out.faces.len() == 1
        {
            tricky_finalize_sel(bm, desel, out.faces[0]);
        } else {
            let faces: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
            let edges: Vec<BMElem> = out.edges.iter().map(|&e| BMElem::Edge(e)).collect();
            edbm::hidden_disable(bm, &faces, true);
            edbm::hidden_disable(bm, &edges, false);
            edbm::select_enable(bm, &faces, true);
            edbm::select_enable(bm, &edges, true);
        }
        changed_multi = true;
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
