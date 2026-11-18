// SPDX-License-Identifier: GPL-3.0-or-later
//! `EDBM_op_*` and `BMO_slot_buffer_hflag_*` equivalents shared by the WP 2.8 operators
//! (`editors/mesh/editmesh_utils.cc`, `bmesh/intern/bmesh_operators.cc`, Blender 5.2.4).
//!
//! - `EDBM_op_finish` reports a `BMO_ERROR_CANCEL` message as INFO (the operator carries on and
//!   nothing changed), a `BMO_ERROR_FATAL` one as ERROR (raised as `RuntimeError` in Python;
//!   frap rolls the statement back, as Blender restores its backup).
//! - Selection writes go through `BM_elem_select_set` when flushing, as Blender's slot
//!   helpers do.

use frap_bmesh::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId};

/// Selected vertices in mesh order (`%hv` with `BM_ELEM_SELECT`).
pub(crate) fn sel_verts(bm: &BMesh) -> Vec<VertId> {
    bm.vert_ids()
        .into_iter()
        .filter(|&v| bm.vert(v).flag.contains(ElemFlag::SELECT))
        .collect()
}

/// Selected edges in mesh order (`%he`).
pub(crate) fn sel_edges(bm: &BMesh) -> Vec<EdgeId> {
    bm.edge_ids()
        .into_iter()
        .filter(|&e| bm.edge(e).flag.contains(ElemFlag::SELECT))
        .collect()
}

/// Selected faces in mesh order (`%hf`).
pub(crate) fn sel_faces(bm: &BMesh) -> Vec<FaceId> {
    bm.face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
        .collect()
}

/// Selected vertices, edges then faces (`%hvef`).
pub(crate) fn sel_geom(bm: &BMesh) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = sel_verts(bm).into_iter().map(BMElem::Vert).collect();
    out.extend(sel_edges(bm).into_iter().map(BMElem::Edge));
    out.extend(sel_faces(bm).into_iter().map(BMElem::Face));
    out
}

/// Edges with a header flag, mesh order (`%he` with another flag).
pub(crate) fn edges_with(bm: &BMesh, flag: ElemFlag) -> Vec<EdgeId> {
    bm.edge_ids()
        .into_iter()
        .filter(|&e| bm.edge(e).flag.contains(flag))
        .collect()
}

/// `BMO_slot_buffer_hflag_enable(..., BM_ELEM_SELECT, do_flush)` on the elements of `htype`.
pub(crate) fn select_enable(bm: &mut BMesh, elems: &[BMElem], flush: bool) {
    for &e in elems {
        if !bm.elem_alive(e) {
            continue;
        }
        if flush {
            bm.select_set(e, true);
        }
        bm.elem_flag_mut(e).insert(ElemFlag::SELECT);
    }
}

/// `BMO_slot_buffer_hflag_disable(..., BM_ELEM_SELECT, do_flush)`.
pub(crate) fn select_disable(bm: &mut BMesh, elems: &[BMElem], flush: bool) {
    for &e in elems {
        if !bm.elem_alive(e) {
            continue;
        }
        if flush {
            bm.select_set(e, false);
        }
        bm.elem_flag_mut(e).remove(ElemFlag::SELECT);
    }
}

/// `BMO_slot_buffer_hflag_disable(..., BM_ELEM_HIDDEN, do_flush)`.
pub(crate) fn hidden_disable(bm: &mut BMesh, elems: &[BMElem], flush: bool) {
    for &e in elems {
        if !bm.elem_alive(e) {
            continue;
        }
        if flush {
            bm.elem_hide_set(e, false);
        }
        bm.elem_flag_mut(e).remove(ElemFlag::HIDDEN);
    }
}

/// `EDBM_flag_disable_all(em, BM_ELEM_SELECT)`.
pub(crate) fn deselect_all(bm: &mut BMesh) {
    super::super::select_all::deselect_all(bm);
}

/// Faces of a `geom`-like list.
pub(crate) fn faces_of(g: &[BMElem]) -> Vec<BMElem> {
    g.iter()
        .copied()
        .filter(|e| matches!(e, BMElem::Face(_)))
        .collect()
}

/// `EDBM_op_finish(do_report = true)` for an op result: `Ok(Some(out))` when it ran,
/// `Ok(None)` after a `BMO_ERROR_CANCEL` (reported as INFO), `Err` for a fatal error.
pub(crate) fn finish<T>(cx: &mut OpCtx, r: Result<T, BmError>) -> Result<Option<T>, OpError> {
    match r {
        Ok(t) => Ok(Some(t)),
        Err(BmError::Cancel(m)) => {
            cx.report(ReportType::Info, m);
            Ok(None)
        }
        Err(e) => Err(OpError::Runtime(e.to_string())),
    }
}

/// `em->mat_nr`: the edit object's active material slot (`actcol - 1`, at least 0).
pub(crate) fn mat_nr(cx: &OpCtx, ob: IdRef<ObjectId>) -> i32 {
    cx.main()
        .objects
        .get(ob)
        .map(|o| o.active_material_index.max(0))
        .unwrap_or(0)
}

/// Read-only view of an edit mesh's BMesh (no generation bump).
pub(crate) fn with_bm<R>(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    f: impl FnOnce(&BMesh) -> R,
) -> Option<R> {
    let (_, edit) = cx.split_edit();
    edit.meshes.get(&me.key()).map(|em| f(&em.bm))
}

/// `edbm_add_edge_face__smooth_get`: most selected edges' first faces are smooth.
pub(crate) fn smooth_get(bm: &BMesh) -> bool {
    let mut vote = [0u32; 2];
    for e in bm.edge_ids() {
        let ed = bm.edge(e);
        if ed.flag.contains(ElemFlag::SELECT)
            && let Some(l) = ed.l
        {
            let f = bm.lp(l).f;
            vote[bm.face(f).flag.contains(ElemFlag::SMOOTH) as usize] += 1;
        }
    }
    vote[0] < vote[1]
}
