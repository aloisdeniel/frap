// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.dissolve_mode(use_verts=False, angle_threshold=π, use_preserve_quads=True,
//! use_face_split=False, use_boundary_tear=False)`, tier T1
//! (`tests/golden/cases/edit/delete/dissolve_mode.py`). Port of `edbm_dissolve_mode_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Dispatches on the select mode of the **active** edit mesh: vertex mode runs
//! `mesh.dissolve_verts`, edge mode `mesh.dissolve_edges`, face mode `mesh.dissolve_faces`
//! (each on every mesh in Edit Mode). An unset `use_verts` becomes `True` unless face select
//! mode is on.

use frap_bmesh::SelectMode;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "dispatches on the active edit mesh's select mode: VERT -> dissolve_verts, EDGE -> dissolve_edges, FACE -> dissolve_faces",
        "an unset use_verts is True unless face select mode is on",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::DissolveMode) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let Some(&(_, active)) = meshes.first() else {
        return Ok(OpStatus::Cancelled);
    };
    let mode = super::delete::common::edit_bm(cx, active)
        .map(|bm| bm.select_mode)
        .unwrap_or_default();
    let mut use_verts = p.use_verts.value;
    if !p.use_verts.is_set && !mode.contains(SelectMode::FACE) {
        // Always enable in edge mode.
        use_verts = true;
    }
    if mode.contains(SelectMode::VERT) {
        return super::dissolve_verts::run(cx, p.use_face_split.value, p.use_boundary_tear.value);
    }
    if mode.contains(SelectMode::EDGE) {
        return super::dissolve_edges::run(
            cx,
            super::dissolve_edges::Args {
                use_verts,
                use_face_split: p.use_face_split.value,
                angle_threshold: p.angle_threshold.value,
                use_preserve_quads: p.use_preserve_quads.value,
            },
        );
    }
    super::dissolve_faces::run(cx, use_verts)
}
