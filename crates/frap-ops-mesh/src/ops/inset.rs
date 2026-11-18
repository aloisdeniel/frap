// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.inset(use_boundary=True, use_even_offset=True, use_relative_offset=False,
//! use_edge_rail=False, thickness=0.0, depth=0.0, use_outset=False, use_select_inset=False,
//! use_individual=False, use_interpolate=True)`, tier T1
//! (`tests/golden/cases/edit/extrude/inset_*.py`). Port of `edbm_inset_exec` /
//! `edbm_inset_calc` (`editors/mesh/editmesh_inset.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Every mesh in Edit Mode with a selected vertex insets its selected faces: as one region
//! (`bmesh.ops.inset_region`; with `use_outset` the hidden faces are excluded) or each face
//! alone (`use_individual`, `bmesh.ops.inset_individual`). Afterwards the selection is
//! cleared (with the history) and the inset faces, or with `use_select_inset` the new ring
//! faces, are selected. With no selected vertex in any mesh the operator is cancelled.
//! `release_confirm` only matters interactively.

use frap_bmesh::ops::inset_individual::{InsetIndividualIn, inset_individual};
use frap_bmesh::ops::inset_region::{InsetRegionIn, inset_region};
use frap_bmesh::{BMesh, ElemFlag, FaceId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::extrude_region::common;

/// `BMO_slot_buffer_from_enabled_hflag(bm, …, BM_FACE, hflag)`: the faces carrying `hflag`
/// (hidden ones only when looking for `HIDDEN`), in iteration order.
fn faces_with(bm: &BMesh, hflag: ElemFlag) -> Vec<FaceId> {
    bm.faces
        .iter()
        .filter(|(_, f)| {
            f.flag.contains(hflag)
                && (hflag.contains(ElemFlag::HIDDEN) || !f.flag.contains(ElemFlag::HIDDEN))
        })
        .map(|(i, _)| FaceId(i))
        .collect()
}

/// `BMO_slot_buffer_hflag_enable(bm, …, BM_FACE, BM_ELEM_SELECT, do_flush = true)`.
fn select_faces(bm: &mut BMesh, faces: &[FaceId]) {
    for &f in faces {
        bm.face_select_set(f, true);
        bm.face_mut(f).flag.insert(ElemFlag::SELECT);
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Inset) -> OpResult {
    let mut changed = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if !common::has_selected_vert(cx, me) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = faces_with(bm, ElemFlag::SELECT);
        let out = if p.use_individual.value {
            inset_individual(
                bm,
                &InsetIndividualIn {
                    faces: faces.clone(),
                    thickness: p.thickness.value,
                    depth: p.depth.value,
                    use_even_offset: p.use_even_offset.value,
                    use_interpolate: p.use_interpolate.value,
                    use_relative_offset: p.use_relative_offset.value,
                },
            )
            .map_err(common::bm_err)?
            .faces
        } else {
            let faces_exclude = if p.use_outset.value {
                faces_with(bm, ElemFlag::HIDDEN)
            } else {
                Vec::new()
            };
            inset_region(
                bm,
                &InsetRegionIn {
                    faces: faces.clone(),
                    faces_exclude,
                    use_boundary: p.use_boundary.value,
                    use_even_offset: p.use_even_offset.value,
                    use_interpolate: p.use_interpolate.value,
                    use_relative_offset: p.use_relative_offset.value,
                    use_edge_rail: p.use_edge_rail.value,
                    thickness: p.thickness.value,
                    depth: p.depth.value,
                    use_outset: p.use_outset.value,
                },
            )
            .map_err(common::bm_err)?
            .faces
        };
        // Deselect the original faces and vertices (`EDBM_flag_disable_all`: the history
        // too), then select the inset faces or the new ones.
        bm.deselect_all();
        if p.use_select_inset.value {
            select_faces(bm, &out);
        } else {
            select_faces(bm, &faces);
        }
        changed = true;
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
