// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.loop_select(delimit_edge_loop={'OUTER_CORNERS', 'NGONS'},
//! delimit_face_loop=set(), extend=False, deselect=False, toggle=False, object_index=-1,
//! edge_index=-1, vert_index=-1, face_index=-1)`, tier T1 (`tests/golden/cases/edit/select/
//! loop_select*.py`). Port of `edbm_select_loop_exec` / `edbm_select_loop_or_ring_exec_impl`,
//! `edbm_select_loop_or_ring_by_edge`, `mouse_mesh_loop_edge`, `mouse_mesh_loop_face`
//! (`editors/mesh/editmesh_select.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Blender's exec (the redo path of the mouse pick) selects by index, which is how agents use
//! it: `object_index` is the position of the object among the objects in Edit Mode (active
//! first), `edge_index` the index of the edge (`mesh.edges` order of the edit-mode view). Both
//! default to -1, so a call without them is `CANCELLED`, as in Blender.
//! - Without `extend`/`deselect`/`toggle` the selection of every object in Edit Mode is
//!   replaced; `deselect` removes the loop; `toggle` removes it when the edge is selected.
//! - Vertex/edge select mode: the edge loop (`BMW_EDGELOOP` with `delimit_edge_loop`; an edge
//!   with more than two faces walks `BMW_EDGELOOP_NONMANIFOLD`). Picking again a boundary loop
//!   that is already selected selects the whole boundary (`BMW_EDGEBOUNDARY`); with a `SEAM`
//!   or `SHARP` delimiter, picking again a loop selected up to its delimiters selects the full
//!   loop.
//! - Face select mode: the face loop (`BMW_FACELOOP` with `delimit_face_loop`).
//! - The edge becomes the active element in edge select mode; `vert_index`/`face_index` make
//!   that vertex/face active when it is selected.

pub(crate) mod walk;

use frap_bmesh::walkers::{LoopDelimit, WalkOpts};
use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};
use frap_spec_ops_mesh::mesh;

use super::select_more::common::{self, Ngons};

/// The pick props shared by `loop_select` and `edgering_select`.
pub(crate) struct Pick {
    pub(crate) extend: bool,
    pub(crate) deselect: bool,
    pub(crate) toggle: bool,
    pub(crate) object_index: i32,
    pub(crate) edge_index: i32,
    pub(crate) vert_index: i32,
    pub(crate) face_index: i32,
}

fn edge_walk(bm: &BMesh, e: EdgeId, delimit: LoopDelimit) -> Vec<BMElem> {
    let o = WalkOpts {
        delimit,
        ..WalkOpts::default()
    };
    bm.walk_edge_loop_ex(e, &o)
        .into_iter()
        .map(BMElem::Edge)
        .collect()
}

fn ring_walk(bm: &BMesh, e: EdgeId, delimit: LoopDelimit) -> Vec<BMElem> {
    let o = WalkOpts {
        delimit,
        ..WalkOpts::default()
    };
    bm.walk_edge_ring_ex(e, &o)
        .into_iter()
        .map(BMElem::Edge)
        .collect()
}

fn edges(v: Vec<EdgeId>) -> Vec<BMElem> {
    v.into_iter().map(BMElem::Edge).collect()
}

/// `mouse_mesh_loop_face`.
fn loop_face(bm: &mut BMesh, e: EdgeId, select: bool, select_clear: bool, delimit: LoopDelimit) {
    if select_clear {
        common::deselect_all(bm);
    }
    let faces: Vec<BMElem> = walk::face_loop(bm, e, delimit)
        .into_iter()
        .map(BMElem::Face)
        .collect();
    common::walker_select(bm, &faces, select);
}

/// The SEAM/SHARP delimiter cycle of `mouse_mesh_loop_edge(_ring)`: true when the walk with
/// the delimiters is already all `select`, but the walk without them is not.
fn full_without_delimiters(
    bm: &BMesh,
    walk: impl Fn(&BMesh, LoopDelimit) -> Vec<BMElem>,
    delimit: LoopDelimit,
    select: bool,
) -> bool {
    let not = usize::from(!select);
    if common::walker_select_count(bm, &walk(bm, delimit))[not] != 0 {
        return false;
    }
    common::walker_select_count(bm, &walk(bm, LoopDelimit::empty()))[not] != 0
}

/// `mouse_mesh_loop_edge_ring`.
fn loop_edge_ring(bm: &mut BMesh, e: EdgeId, select: bool, select_clear: bool, d: LoopDelimit) {
    let mut full_loop = false;
    if d.intersects(LoopDelimit::SEAM | LoopDelimit::SHARP) {
        full_loop = full_without_delimiters(bm, |bm, d| ring_walk(bm, e, d), d, select);
    }
    if select_clear {
        common::deselect_all(bm);
    }
    let d = if full_loop { LoopDelimit::empty() } else { d };
    let elems = ring_walk(bm, e, d);
    common::walker_select(bm, &elems, select);
}

/// `mouse_mesh_loop_edge`.
fn loop_edge(
    bm: &mut BMesh,
    e: EdgeId,
    select: bool,
    select_clear: bool,
    select_cycle: bool,
    d: LoopDelimit,
) {
    let mut full_boundary = false;
    let mut full_loop = false;
    let non_manifold = bm.edge_face_count(e) > 2;
    let not = usize::from(!select);
    if select_cycle && bm.edge_is_boundary(e) {
        // If the loop is selected, toggle to the boundary; if the boundary is, back to the loop.
        if common::walker_select_count(bm, &edge_walk(bm, e, d))[not] == 0 {
            full_boundary = common::walker_select_count(bm, &edges(walk::edge_boundary(bm, e)))
                [not]
                != 0;
        }
    } else if !non_manifold && d.intersects(LoopDelimit::SEAM | LoopDelimit::SHARP) {
        full_loop = full_without_delimiters(bm, |bm, d| edge_walk(bm, e, d), d, select);
    }
    if select_clear {
        common::deselect_all(bm);
    }
    let elems = if full_boundary {
        edges(walk::edge_boundary(bm, e))
    } else if non_manifold {
        edges(walk::edge_loop_nonmanifold(bm, e))
    } else if full_loop {
        edge_walk(bm, e, LoopDelimit::empty())
    } else {
        edge_walk(bm, e, d)
    };
    common::walker_select(bm, &elems, select);
}

/// The select mode of the context edit object's mesh (`CTX_data_edit_object` → `em->selectmode`).
pub(crate) fn edit_select_mode(cx: &mut OpCtx) -> SelectMode {
    let me = {
        let c = cx.ctx();
        poll::edit_object(&c).and_then(|(_, o)| o.data?.typed::<MeshId>())
    };
    me.and_then(|m| common::read(cx, m, |bm| bm.select_mode))
        .unwrap_or_default()
}

/// `edbm_select_loop_or_ring_exec_impl` (+ `edbm_select_loop_or_ring_by_edge`).
pub(crate) fn exec_impl(cx: &mut OpCtx, p: &Pick, ring: bool, delimit: LoopDelimit) -> OpResult {
    // Intentionally wrap negative values so the lookup fails (as Blender's `uint` casts).
    let object_index = p.object_index as u32 as usize;
    let edge_index = p.edge_index as u32 as usize;
    let bases: Vec<IdRef<MeshId>> = {
        let c = cx.ctx();
        c.objects_in_mode()
            .into_iter()
            .filter_map(|r| c.main.objects.get(r)?.data?.typed::<MeshId>())
            .collect()
    };
    let Some(&me) = bases.get(object_index) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(Some(eed)) =
        common::read(cx, me, |bm| bm.edge_ids().get(edge_index).copied())
    else {
        return Ok(OpStatus::Cancelled);
    };
    let (eed_hidden, eed_selected) =
        common::read(cx, me, |bm| (common::hidden(bm, eed), common::selected(bm, eed)))
            .unwrap_or((true, false));
    if eed_hidden {
        return Ok(OpStatus::Cancelled);
    }
    let mut select = true;
    let mut select_clear = false;
    let mut select_cycle = true;
    if !p.extend && !p.deselect && !p.toggle {
        select_clear = true;
    }
    if p.extend {
        select = true;
    } else if p.deselect {
        select = false;
    } else if select_clear || !eed_selected {
        select = true;
    } else if p.toggle {
        select = false;
        select_cycle = false;
    }
    // Clear the selection of the other objects in Edit Mode when not extending (the current
    // object is cleared later: the cycle checks read its selection first).
    if select_clear {
        let me_obj = {
            let c = cx.ctx();
            c.objects_in_mode().get(object_index).copied()
        };
        let others: Vec<IdRef<MeshId>> = {
            let c = cx.ctx();
            c.objects_in_mode()
                .into_iter()
                .filter(|&r| Some(r) != me_obj)
                .filter_map(|r| c.main.objects.get(r)?.data?.typed::<MeshId>())
                .collect()
        };
        for other in others {
            let any = common::read(cx, other, |bm| bm.selected_counts().0 != 0).unwrap_or(false);
            if any {
                common::deselect_all(cx.bmesh_mut(other)?);
            }
        }
    }
    let bm = cx.bmesh_mut(me)?;
    let mode = bm.select_mode;
    if mode.contains(SelectMode::FACE) {
        loop_face(bm, eed, select, select_clear, delimit);
    } else if ring {
        loop_edge_ring(bm, eed, select, select_clear, delimit);
    } else {
        loop_edge(bm, eed, select, select_clear, select_cycle, delimit);
    }
    common::selectmode_flush(bm);
    // Sets as active, useful for other tools.
    if select && mode.contains(SelectMode::EDGE) {
        bm.select_history_store_if_absent(BMElem::Edge(eed));
    }
    // Set the active element from the stored indices (redo support).
    if select {
        let vert_index = p.vert_index as u32 as usize;
        let face_index = p.face_index as u32 as usize;
        if let Some(v) = bm.vert_ids().get(vert_index).copied() {
            let f = bm.vert(v).flag;
            if !f.contains(ElemFlag::HIDDEN) && f.contains(ElemFlag::SELECT) {
                bm.select_history_store_if_absent(BMElem::Vert(VertId(v.0)));
            }
        } else if let Some(f) = bm.face_ids().get(face_index).copied() {
            let fl = bm.face(f).flag;
            if !fl.contains(ElemFlag::HIDDEN) && fl.contains(ElemFlag::SELECT) {
                bm.act_face = Some(FaceId(f.0));
                bm.select_history_store_if_absent(BMElem::Face(f));
            }
        }
    }
    Ok(OpStatus::Finished)
}

#[operator(
    poll = poll::EDITMESH_VIEW3D,
    tier = T1,
    notes = [
        "select by index like Blender's redo path: object_index (objects in Edit Mode, active first) and edge_index (mesh.edges order) are required; without them the operator is CANCELLED",
        "face select mode walks the face loop across the edge (delimit_face_loop), other modes the edge loop (delimit_edge_loop)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::LoopSelect) -> OpResult {
    let face_mode = edit_select_mode(cx).contains(SelectMode::FACE);
    let delimit = if face_mode {
        common::walk_delimit(&p.delimit_face_loop.value, Ngons::Loop)
    } else {
        common::walk_delimit(&p.delimit_edge_loop.value, Ngons::Loop)
    };
    let pick = Pick {
        extend: p.extend.value,
        deselect: p.deselect.value,
        toggle: p.toggle.value,
        object_index: p.object_index.value,
        edge_index: p.edge_index.value,
        vert_index: p.vert_index.value,
        face_index: p.face_index.value,
    };
    exec_impl(cx, &pick, false, delimit)
}
