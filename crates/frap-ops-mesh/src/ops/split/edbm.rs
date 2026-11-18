// SPDX-License-Identifier: GPL-3.0-or-later
//! The `EDBM_*` / `BMO_slot_buffer_*` glue the WP 2.10 edit-mode operators share (split,
//! edge_split, separate, duplicate, triangulate, poke, spin, screw, solidify, wireframe,
//! symmetrize, vertices_smooth, flip_quad_tessellation): gathering the selection the way the
//! `%hv`/`%he`/`%hf`/`%hvef` format codes do with `BMO_FLAG_RESPECT_HIDE`, selecting an
//! operator's output, and turning a bmesh operator error into Blender's report.
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_utils.cc (`EDBM_op_finish`,
//! `EDBM_op_call_and_selectf`, `EDBM_flag_disable_all`) and
//! blender/source/blender/bmesh/intern/bmesh_operators.cc (`BMO_slot_buffer_hflag_enable`,
//! `BMO_slot_buffer_hflag_disable`), Blender 5.2.4.

use frap_bmesh::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};

/// Reads the edit BMesh of `me` without marking it changed (no `bm_gen` bump): `None` when
/// the mesh is not in Edit Mode.
pub(crate) fn read<R>(cx: &mut OpCtx, me: IdRef<MeshId>, f: impl FnOnce(&BMesh) -> R) -> Option<R> {
    let (_, edit) = cx.split_edit();
    edit.meshes.get(&me.key()).map(|em| f(&em.bm))
}

fn usable(flag: ElemFlag) -> bool {
    flag.contains(ElemFlag::SELECT) && !flag.contains(ElemFlag::HIDDEN)
}

/// `%hv` with `BM_ELEM_SELECT` (hidden vertices excluded): selected vertices, vertex order.
pub(crate) fn selected_verts(bm: &BMesh) -> Vec<VertId> {
    bm.verts
        .iter()
        .filter(|(_, v)| usable(v.flag))
        .map(|(i, _)| VertId(i))
        .collect()
}

/// `%he` with `BM_ELEM_SELECT`: selected edges, edge order.
pub(crate) fn selected_edges(bm: &BMesh) -> Vec<EdgeId> {
    bm.edges
        .iter()
        .filter(|(_, e)| usable(e.flag))
        .map(|(i, _)| EdgeId(i))
        .collect()
}

/// `%hf` with `BM_ELEM_SELECT`: selected faces, face order.
pub(crate) fn selected_faces(bm: &BMesh) -> Vec<FaceId> {
    bm.faces
        .iter()
        .filter(|(_, f)| usable(f.flag))
        .map(|(i, _)| FaceId(i))
        .collect()
}

/// `%hvef` with `BM_ELEM_SELECT`: selected vertices, then edges, then faces.
pub(crate) fn selected_geom(bm: &BMesh) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = selected_verts(bm).into_iter().map(BMElem::Vert).collect();
    out.extend(selected_edges(bm).into_iter().map(BMElem::Edge));
    out.extend(selected_faces(bm).into_iter().map(BMElem::Face));
    out
}

/// `BM_mesh_elem_hflag_disable_all(bm, BM_VERT | BM_EDGE | BM_FACE, BM_ELEM_SELECT, false)`
/// (= `EDBM_flag_disable_all(em, BM_ELEM_SELECT)`): every element deselected, hidden ones
/// included, and the selection history cleared.
pub(crate) fn deselect_all(bm: &mut BMesh) {
    crate::ops::select_all::deselect_all(bm);
}

/// `BMO_slot_buffer_hflag_enable(bm, slot, htype, BM_ELEM_SELECT, do_flush = true)`:
/// `BM_elem_select_set` on every element (hidden ones stay unselected).
pub(crate) fn select_elems(bm: &mut BMesh, elems: &[BMElem]) {
    for &e in elems {
        if bm.elem_alive(e) {
            bm.select_set(e, true);
        }
    }
}

/// `BMO_slot_buffer_hflag_disable(bm, slot, htype, BM_ELEM_SELECT, do_flush = true)`.
pub(crate) fn deselect_elems(bm: &mut BMesh, elems: &[BMElem]) {
    for &e in elems {
        if bm.elem_alive(e) {
            bm.select_set(e, false);
        }
    }
}

/// `EDBM_op_finish` for a bmesh operator result: `Ok(true)` when the operator ran
/// (`changed`), `Ok(false)` when it was cancelled (its message becomes an INFO report, as
/// `EDBM_op_finish` reports `BMO_ERROR_CANCEL` at `RPT_INFO`). A fatal error is an ERROR
/// report, which Python raises as `RuntimeError` (`Error: …`); invalid input is a frap bug.
pub(crate) fn finish<T>(cx: &mut OpCtx, r: Result<T, BmError>) -> Result<Option<T>, OpError> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(BmError::Cancel(msg)) => {
            cx.report(ReportType::Info, msg);
            Ok(None)
        }
        Err(BmError::Fatal(msg)) => Err(OpError::Runtime(msg)),
        Err(e) => Err(OpError::Runtime(format!("frap: internal bmesh error: {e}"))),
    }
}

/// `(totvertsel, totedgesel, totfacesel)` of the edit mesh.
pub(crate) fn sel_counts(bm: &BMesh) -> (usize, usize, usize) {
    bm.selected_counts()
}
