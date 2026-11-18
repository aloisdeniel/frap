// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_less(use_face_step=True)`, tier T1 (`tests/golden/cases/edit/select/
//! more_less*.py`). Port of `edbm_select_less_exec`, `EDBM_select_less` and
//! `BM_mesh_select_mode_clean` (`editors/mesh/editmesh_select.cc`, `editmesh_utils.cc`,
//! `bmesh/intern/bmesh_marking.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `bmesh.ops.region_extend(use_contract=True)`.
//!
//! For every mesh in Edit Mode with a selection: the selected elements on the border of each
//! selected region are deselected (in face-only select mode, faces next to an unselected face;
//! otherwise vertices next to an unselected edge, or face with `use_face_step`, with all their
//! edges), then isolated leftovers are cleaned for the select mode.

use frap_bmesh::{BMesh, ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::{common, region_step};

// Ported from blender/source/blender/bmesh/intern/bmesh_marking.cc (BM_mesh_select_mode_clean_ex).
/// `BM_mesh_select_mode_clean`: in edge mode, vertices follow the selected edges; in face
/// mode, vertices and edges follow the selected faces.
pub(crate) fn select_mode_clean(bm: &mut BMesh) {
    let mode = bm.select_mode;
    if mode.contains(SelectMode::VERT) {
        return;
    }
    let (tv, te, tf) = bm.selected_counts();
    if mode.contains(SelectMode::EDGE) {
        if tv != 0 {
            for v in bm.vert_ids() {
                bm.vert_mut(v).flag.remove(ElemFlag::SELECT);
            }
        }
        if te != 0 {
            for e in bm.edge_ids() {
                if bm.is_selected(e.into()) {
                    let [a, b] = bm.edge(e).v;
                    bm.vert_select_set(a, true);
                    bm.vert_select_set(b, true);
                }
            }
        }
    } else if mode.contains(SelectMode::FACE) {
        if tv != 0 {
            for v in bm.vert_ids() {
                bm.vert_mut(v).flag.remove(ElemFlag::SELECT);
            }
        }
        if te != 0 {
            for e in bm.edge_ids() {
                bm.edge_mut(e).flag.remove(ElemFlag::SELECT);
            }
        }
        if tf != 0 {
            for f in bm.face_ids() {
                if bm.is_selected(f.into()) {
                    for e in bm.face_edges(f) {
                        bm.edge_select_set(e, true);
                    }
                }
            }
        }
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectLess) -> OpResult {
    for (_, me) in common::edit_meshes(cx) {
        if !common::read(cx, me, common::any_selected).unwrap_or(false) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        region_step(bm, true, p.use_face_step.value)?;
        // Only needed for select less: no isolated elements may remain.
        select_mode_clean(bm);
    }
    Ok(OpStatus::Finished)
}
