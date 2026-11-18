// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_all(action=…)`, tier T1 (`tests/golden/cases/edit/prereq/
//! select_all_*.py`). Port of `edbm_select_all_exec` (`editors/mesh/editmesh_select.cc`),
//! `EDBM_flag_enable_all` / `EDBM_flag_disable_all` (`editmesh_utils.cc`), `EDBM_select_swap`
//! and `BM_mesh_elem_hflag_enable_test` / `BM_mesh_elem_hflag_disable_test`
//! (`bmesh/intern/bmesh_marking.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Every mesh in Edit Mode takes part (multi-object edit, one entry per mesh):
//! - `TOGGLE` deselects everything when any vertex, edge or face of any of them is selected,
//!   else selects everything;
//! - `SELECT` selects every visible vertex, then edge, then face (`BM_elem_select_set`, so an
//!   edge selects its vertices); the selection history is kept;
//! - `DESELECT` clears the selection of every element, hidden ones included, and the selection
//!   history;
//! - `INVERT` toggles the visible elements of the highest-priority select mode (vertices, else
//!   edges, else faces), then flushes the selection for the mode.
//!
//! The operator always finishes, even when nothing changes.

use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, select_all::Action};

/// Whether anything is selected (`bm->totvertsel || bm->totedgesel || bm->totfacesel`).
pub(crate) fn any_selected(bm: &BMesh) -> bool {
    let (v, e, f) = bm.selected_counts();
    v + e + f > 0
}

/// Every live element of a pool, as ids (collected so the BMesh can be changed while looping).
fn verts(bm: &BMesh) -> Vec<VertId> {
    bm.verts.iter().map(|(i, _)| VertId(i)).collect()
}
fn edges(bm: &BMesh) -> Vec<EdgeId> {
    bm.edges.iter().map(|(i, _)| EdgeId(i)).collect()
}
fn faces(bm: &BMesh) -> Vec<FaceId> {
    bm.faces.iter().map(|(i, _)| FaceId(i)).collect()
}

/// The header flags of an element.
fn flag(bm: &BMesh, e: BMElem) -> ElemFlag {
    match e {
        BMElem::Vert(v) => bm.verts.get(v.0).map(|x| x.flag),
        BMElem::Edge(x) => bm.edges.get(x.0).map(|x| x.flag),
        BMElem::Face(f) => bm.faces.get(f.0).map(|x| x.flag),
    }
    .unwrap_or_default()
}

/// Every live element: vertices, then edges, then faces (`BM_mesh_elem_hflag_*_test` order).
fn all_elems(bm: &BMesh) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = verts(bm).into_iter().map(BMElem::Vert).collect();
    out.extend(edges(bm).into_iter().map(BMElem::Edge));
    out.extend(faces(bm).into_iter().map(BMElem::Face));
    out
}

/// `EDBM_flag_enable_all(em, BM_ELEM_SELECT)`: hidden elements are skipped.
pub(crate) fn select_all(bm: &mut BMesh) {
    for e in all_elems(bm) {
        if !flag(bm, e).contains(ElemFlag::HIDDEN) {
            bm.select_set(e, true);
        }
    }
}

/// `EDBM_flag_disable_all(em, BM_ELEM_SELECT)`: the fast path of
/// `BM_mesh_elem_hflag_disable_test` (every element, hidden ones too; history cleared).
pub(crate) fn deselect_all(bm: &mut BMesh) {
    bm.select_history.clear();
    for v in bm.verts.slots.iter_mut().flatten() {
        v.flag.remove(ElemFlag::SELECT);
    }
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(ElemFlag::SELECT);
    }
    for f in bm.faces.slots.iter_mut().flatten() {
        f.flag.remove(ElemFlag::SELECT);
    }
}

/// `EDBM_select_swap` then `EDBM_selectmode_flush`.
pub(crate) fn invert(bm: &mut BMesh) {
    let elems: Vec<BMElem> = if bm.select_mode.contains(SelectMode::VERT) {
        verts(bm).into_iter().map(BMElem::Vert).collect()
    } else if bm.select_mode.contains(SelectMode::EDGE) {
        edges(bm).into_iter().map(BMElem::Edge).collect()
    } else {
        faces(bm).into_iter().map(BMElem::Face).collect()
    };
    for e in elems {
        let f = flag(bm, e);
        if f.contains(ElemFlag::HIDDEN) {
            continue;
        }
        bm.select_set(e, !f.contains(ElemFlag::SELECT));
    }
    bm.select_flush_mode();
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectAll) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let action = match p.action.value {
        Action::Toggle => {
            let (_, edit) = cx.split_edit();
            let any = meshes.iter().any(|(_, me)| {
                edit.meshes
                    .get(&me.key())
                    .is_some_and(|em| any_selected(&em.bm))
            });
            if any {
                Action::Deselect
            } else {
                Action::Select
            }
        }
        a => a,
    };
    for (_, me) in meshes {
        let bm = cx.bmesh_mut(me)?;
        match action {
            Action::Select => select_all(bm),
            Action::Deselect => deselect_all(bm),
            Action::Invert => invert(bm),
            Action::Toggle => unreachable!("resolved above"),
        }
    }
    Ok(OpStatus::Finished)
}
