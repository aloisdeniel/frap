// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.hide(unselected=False)`, tier T1 (`tests/golden/cases/edit/select/
//! hide_reveal*.py`). Port of `edbm_hide_exec` (`editors/mesh/editmesh_tools.cc`) and
//! `EDBM_mesh_hide` (`editors/mesh/editmesh_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Per mesh in Edit Mode, the selected (or with `unselected`, the unselected) elements of the
//! highest select mode (vertices, else edges, else faces) are deselected and hidden; hiding
//! flushes up (a hidden vertex hides its edges and faces, a hidden edge its faces) and, for
//! edges and faces, down to elements left without a visible user. Hiding the unselected in
//! face mode also hides unselected loose edges, and in edge/face mode unselected loose
//! vertices. A mesh with nothing to hide (nothing selected, or everything selected with
//! `unselected`) is skipped; when no mesh changed the operator is `CANCELLED`. Hidden elements
//! are skipped by every other operator until `mesh.reveal`.

use frap_bmesh::{BMElem, BMesh, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;

/// `EDBM_mesh_hide(em, swap)`: returns whether anything was hidden.
pub(crate) fn mesh_hide(bm: &mut BMesh, swap: bool) -> bool {
    let mode = common::primary_mode(bm.select_mode);
    let elems: Vec<BMElem> = if mode == SelectMode::VERT {
        bm.vert_ids().into_iter().map(BMElem::Vert).collect()
    } else if mode == SelectMode::EDGE {
        bm.edge_ids().into_iter().map(BMElem::Edge).collect()
    } else {
        bm.face_ids().into_iter().map(BMElem::Face).collect()
    };
    let mut changed = false;
    for e in elems {
        if !common::hidden(bm, e) && (common::selected(bm, e) != swap) {
            bm.elem_hide_set(e, true);
            changed = true;
        }
    }
    // Hiding unselected.
    if swap {
        // In face select mode, also hide loose edges that aren't part of any visible face.
        if mode == SelectMode::FACE {
            for e in bm.edge_ids() {
                if !bm.edge_is_wire(e) {
                    continue;
                }
                if !common::hidden(bm, e) && !common::selected(bm, e) {
                    bm.elem_hide_set(BMElem::Edge(e), true);
                    changed = true;
                }
            }
        }
        // In edge or face select mode, also hide isolated vertices.
        if mode != SelectMode::VERT {
            for v in bm.vert_ids() {
                if bm.vert(v).e.is_some() {
                    continue;
                }
                if !common::hidden(bm, v) && !common::selected(bm, v) {
                    bm.elem_hide_set(BMElem::Vert(v), true);
                    changed = true;
                }
            }
        }
    }
    if changed {
        common::selectmode_flush(bm);
    }
    changed
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Hide) -> OpResult {
    let unselected = p.unselected.value;
    let mut changed = false;
    for (_, me) in common::edit_meshes(cx) {
        let skip = common::read(cx, me, |bm| {
            let (tv, te, tf) = bm.selected_counts();
            if unselected {
                match common::primary_mode(bm.select_mode) {
                    m if m == SelectMode::VERT => tv == bm.verts_num(),
                    m if m == SelectMode::EDGE => te == bm.edges_num(),
                    _ => tf == bm.faces_num(),
                }
            } else {
                tv == 0
            }
        })
        .unwrap_or(true);
        if skip {
            continue;
        }
        if mesh_hide(cx.bmesh_mut(me)?, unselected) {
            changed = true;
        }
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
