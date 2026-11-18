// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.normals_make_consistent(inside=False)`, tier T1
//! (`tests/golden/cases/edit/normals/make_consistent_*.py`). Port of
//! `edbm_normals_make_consistent_exec` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Every mesh in Edit Mode (unique data) with a selected face runs
//! `bmesh.ops.recalc_face_normals` on its selected faces: each group of faces connected across
//! manifold edges gets a consistent winding pointing away from its centre (only selected faces
//! are flipped). `inside=True` then reverses the selected faces (custom normals follow, as
//! with `mesh.flip_normals`). The selection is unchanged; the operator always finishes.

use frap_bmesh::ops::recalc_face_normals::{RecalcFaceNormalsIn, recalc_face_normals};
use frap_bmesh::ops::reverse_faces::{ReverseFacesIn, reverse_faces};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{bm_err, clnor, edit_meshes_where, sel};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::NormalsMakeConsistent) -> OpResult {
    let inside = p.inside.value;
    for me in edit_meshes_where(cx, |bm| sel(bm).2 > 0) {
        let bm = cx.bmesh_mut(me)?;
        let saved = if inside { clnor::flip_init(bm) } else { None };
        let faces = clnor::selected_faces(bm);
        recalc_face_normals(
            bm,
            &RecalcFaceNormalsIn {
                faces: faces.clone(),
            },
        )
        .map_err(bm_err)?;
        if inside {
            reverse_faces(
                bm,
                &ReverseFacesIn {
                    faces,
                    flip_multires: true,
                },
            )
            .map_err(bm_err)?;
            clnor::flip_apply(bm, saved);
        }
    }
    Ok(OpStatus::Finished)
}
