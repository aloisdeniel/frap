// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.reveal(select=True)`, tier T1 (`tests/golden/cases/edit/select/
//! hide_reveal*.py`). Port of `edbm_reveal_exec` (`editors/mesh/editmesh_tools.cc`) and
//! `EDBM_mesh_reveal` (`editors/mesh/editmesh_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Per mesh in Edit Mode with hidden elements: everything is revealed, and the elements that
//! were hidden are selected (`select=True`) or deselected (`select=False`), for each enabled
//! select mode in turn (vertices, edges, faces); the selection is then flushed and the normals
//! recomputed. The operator always finishes.

use frap_bmesh::{BMElem, BMesh, ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;

/// `EDBM_mesh_reveal(em, select)`: returns whether anything was hidden.
pub(crate) fn mesh_reveal(bm: &mut BMesh, select: bool) -> bool {
    // `BM_ELEM_HIDDEN` → `BM_ELEM_TAG`: remember what was hidden.
    let was_hidden = |bm: &BMesh, all: Vec<BMElem>| -> Vec<BMElem> {
        all.into_iter().filter(|&e| common::hidden(bm, e)).collect()
    };
    let verts = was_hidden(bm, bm.vert_ids().into_iter().map(BMElem::Vert).collect());
    let edges = was_hidden(bm, bm.edge_ids().into_iter().map(BMElem::Edge).collect());
    let faces = was_hidden(bm, bm.face_ids().into_iter().map(BMElem::Face).collect());
    if verts.is_empty() && edges.is_empty() && faces.is_empty() {
        return false;
    }
    // Reveal everything (`EDBM_flag_disable_all(em, BM_ELEM_HIDDEN)`).
    for v in bm.verts.slots.iter_mut().flatten() {
        v.flag.remove(ElemFlag::HIDDEN);
    }
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(ElemFlag::HIDDEN);
    }
    for f in bm.faces.slots.iter_mut().flatten() {
        f.flag.remove(ElemFlag::HIDDEN);
    }
    // Select relevant just-revealed elements.
    let mode = bm.select_mode;
    for (bit, elems) in [
        (SelectMode::VERT, verts),
        (SelectMode::EDGE, edges),
        (SelectMode::FACE, faces),
    ] {
        if !mode.contains(bit) {
            continue;
        }
        for e in elems {
            bm.select_set(e, select);
        }
    }
    common::selectmode_flush(bm);
    // Hidden faces can have invalid normals.
    bm.normals_update();
    true
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Reveal) -> OpResult {
    for (_, me) in common::edit_meshes(cx) {
        let any_hidden = common::read(cx, me, |bm| {
            bm.verts.iter().any(|(_, v)| v.flag.contains(ElemFlag::HIDDEN))
                || bm.edges.iter().any(|(_, e)| e.flag.contains(ElemFlag::HIDDEN))
                || bm.faces.iter().any(|(_, f)| f.flag.contains(ElemFlag::HIDDEN))
        })
        .unwrap_or(false);
        if any_hidden {
            mesh_reveal(cx.bmesh_mut(me)?, p.select.value);
        }
    }
    Ok(OpStatus::Finished)
}
