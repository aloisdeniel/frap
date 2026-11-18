// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.flip_normals(only_clnors=False)`, tier T1
//! (`tests/golden/cases/edit/normals/flip_*.py`). Port of `edbm_flip_normals_exec`,
//! `edbm_flip_normals_face_winding` and `edbm_flip_normals_custom_loop_normals`
//! (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Every mesh in Edit Mode (unique data) with a selected face has its selected faces reversed
//! with `bmesh.ops.reverse_faces` ([MESH §4.9]: the first corner is kept, `[7,3,1,5]` becomes
//! `[7,5,1,3]`; UVs and other corner data stay on their corner). Custom normals
//! (`custom_normal`) are flipped with the faces: the normals around the selected faces are
//! re-encoded in the new fan spaces, those of the selected faces negated.
//!
//! `only_clnors=True` leaves the winding alone and negates the custom normals of the selected
//! elements (by select mode and selection history, whole smooth fans), on meshes with any
//! selection; a mesh without custom normals is left as it is.
//!
//! The selection and its history are unchanged; the operator always finishes.
//!
//! [`clnor`] holds the custom-normal machinery the normal operators share.

use frap_bmesh::BMesh;
use frap_bmesh::ops::reverse_faces::{ReverseFacesIn, reverse_faces};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};
use frap_spec_ops_mesh::mesh;

pub(crate) mod clnor;

/// The meshes in Edit Mode (unique data, active first) whose BMesh passes `test`, read without
/// touching them.
pub(crate) fn edit_meshes_where(
    cx: &mut OpCtx,
    test: impl Fn(&BMesh) -> bool,
) -> Vec<IdRef<MeshId>> {
    let meshes = helpers::iter_edit_meshes(cx);
    let (_, edit) = cx.split_edit();
    meshes
        .into_iter()
        .filter(|(_, me)| edit.meshes.get(&me.key()).is_some_and(|em| test(&em.bm)))
        .map(|(_, me)| me)
        .collect()
}

/// `(totvertsel, totedgesel, totfacesel)`.
pub(crate) fn sel(bm: &BMesh) -> (usize, usize, usize) {
    bm.selected_counts()
}

/// Converts a BMesh operator error to an operator error.
pub(crate) fn bm_err(e: frap_bmesh::BmError) -> OpError {
    OpError::Runtime(e.to_string())
}

/// `edbm_flip_normals_face_winding`: reverses the selected faces, carrying custom normals.
pub(crate) fn flip_face_winding(bm: &mut BMesh) -> Result<(), OpError> {
    let saved = clnor::flip_init(bm);
    let faces = clnor::selected_faces(bm);
    reverse_faces(
        bm,
        &ReverseFacesIn {
            faces,
            flip_multires: true,
        },
    )
    .map_err(bm_err)?;
    clnor::flip_apply(bm, saved);
    Ok(())
}

/// `edbm_flip_normals_custom_loop_normals`.
fn flip_custom_loop_normals(bm: &mut BMesh) {
    if !clnor::has_layer(bm) {
        return;
    }
    let sp = clnor::update(bm);
    let arr = clnor::editdata_array(bm, &sp, false, clnor::Mark::Indiv);
    for d in arr.data {
        let n = [-d.nloc[0], -d.nloc[1], -d.nloc[2]];
        let v = sp.encode(d.loop_, n);
        clnor::set(bm, d.loop_, v);
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::FlipNormals) -> OpResult {
    let only_clnors = p.only_clnors.value;
    let meshes = edit_meshes_where(cx, |bm| {
        let (v, e, f) = sel(bm);
        if only_clnors { v + e + f > 0 } else { f > 0 }
    });
    for me in meshes {
        let bm = cx.bmesh_mut(me)?;
        if only_clnors {
            flip_custom_loop_normals(bm);
        } else {
            flip_face_winding(bm)?;
        }
    }
    Ok(OpStatus::Finished)
}
