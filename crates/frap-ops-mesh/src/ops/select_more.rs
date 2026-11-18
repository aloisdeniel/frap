// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_more(use_face_step=True)`, tier T1 (`tests/golden/cases/edit/select/
//! more_less*.py`). Port of `edbm_select_more_exec` and `EDBM_select_more`
//! (`editors/mesh/editmesh_select.cc`, `editmesh_utils.cc`, Blender 5.2.4, GPL-2.0-or-later)
//! over `bmesh.ops.region_extend` (`frap_bmesh::ops::region_extend`).
//!
//! For every mesh in Edit Mode with a selection: in face-only select mode the faces next to
//! the selected ones (across an edge, or a vertex with `use_face_step`) are selected; in the
//! other modes the vertices one edge away (with `use_face_step`, every vertex of a face
//! touching the selection) and the edges/faces they complete. The selection history is kept
//! (minus unselected entries).

pub(crate) mod common;

use frap_bmesh::ops::region_extend::{RegionExtendIn, region_extend};
use frap_bmesh::{BMElem, BMesh, BmError, ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

/// `BMO_slot_buffer_from_enabled_hflag(…, "geom", BM_VERT | BM_EDGE | BM_FACE, BM_ELEM_SELECT)`.
fn selected_geom(bm: &BMesh) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = bm
        .vert_ids()
        .into_iter()
        .map(BMElem::Vert)
        .filter(|&e| bm.is_selected(e))
        .collect();
    out.extend(
        bm.edge_ids()
            .into_iter()
            .map(BMElem::Edge)
            .filter(|&e| bm.is_selected(e)),
    );
    out.extend(
        bm.face_ids()
            .into_iter()
            .map(BMElem::Face)
            .filter(|&e| bm.is_selected(e)),
    );
    out
}

fn op_error(e: BmError) -> OpError {
    OpError::Runtime(e.to_string())
}

/// `EDBM_select_more` (`contract = false`) / `EDBM_select_less` (`contract = true`) before the
/// final `BM_mesh_select_mode_clean` of `select_less`.
pub(crate) fn region_step(bm: &mut BMesh, contract: bool, use_face_step: bool) -> OpResult {
    let use_faces = bm.select_mode == SelectMode::FACE;
    let out = region_extend(
        bm,
        &RegionExtendIn {
            geom: selected_geom(bm),
            use_contract: contract,
            use_faces,
            use_face_step,
        },
    )
    .map_err(op_error)?;
    // `BMO_slot_buffer_hflag_enable/disable(…, BM_ELEM_SELECT, do_flush = use_faces)`: flushed
    // (`BM_elem_select_set`) only in face mode; otherwise the flag alone, never on hidden
    // elements when selecting.
    for e in out.geom {
        if use_faces {
            bm.select_set(e, !contract);
        }
        let f = bm.elem_flag_mut(e);
        if contract {
            f.remove(ElemFlag::SELECT);
        } else if !f.contains(ElemFlag::HIDDEN) {
            f.insert(ElemFlag::SELECT);
        }
    }
    common::selectmode_flush(bm);
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectMore) -> OpResult {
    for (_, me) in common::edit_meshes(cx) {
        if !common::read(cx, me, common::any_selected).unwrap_or(false) {
            continue;
        }
        region_step(cx.bmesh_mut(me)?, false, p.use_face_step.value)?;
    }
    Ok(OpStatus::Finished)
}
