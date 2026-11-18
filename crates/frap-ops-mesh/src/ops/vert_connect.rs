// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.vert_connect()` ("J" on two vertices of a face), tier T1
//! (`tests/golden/cases/edit/create/vert_connect*.py`). Port of `edbm_vert_connect_exec` and
//! `edbm_connect_vert_pair` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Exactly two selected vertices that share no face are connected across faces with
//! `bmesh.ops.connect_vert_pair` (new vertices are created where the path crosses edges, and
//! the new edges are selected); otherwise the selected vertices are joined inside the faces
//! they share with `bmesh.ops.connect_verts(check_degenerate)`. A failed connection restores
//! the mesh. `CANCELLED` when no mesh connected anything.

use frap_bmesh::ops::connect_vert_pair::{ConnectVertPairIn, connect_vert_pair};
use frap_bmesh::ops::connect_verts::{ConnectVertsIn, connect_verts};
use frap_bmesh::{BMElem, BMesh, ElemFlag, FaceId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

/// `BM_vert_pair_share_face_check_cb` with "face not hidden".
fn pair_share_visible_face(bm: &BMesh, a: VertId, b: VertId) -> bool {
    if bm.vert(a).e.is_none() || bm.vert(b).e.is_none() {
        return false;
    }
    bm.vert_faces(a).into_iter().any(|f| {
        !bm.face(f).flag.contains(ElemFlag::HIDDEN) && bm.face_vert_share_loop(f, b).is_some()
    })
}

fn hidden_verts(bm: &BMesh) -> Vec<VertId> {
    bm.vert_ids()
        .into_iter()
        .filter(|&v| bm.vert(v).flag.contains(ElemFlag::HIDDEN))
        .collect()
}

fn hidden_faces(bm: &BMesh) -> Vec<FaceId> {
    bm.face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::HIDDEN))
        .collect()
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc (edbm_connect_vert_pair).
/// Connects the selected vertices of one edit mesh; the number of edges made (0 = failure,
/// the mesh is restored).
pub(crate) fn connect_vert_pair_mesh(cx: &mut OpCtx, me: IdRef<MeshId>) -> Result<usize, OpError> {
    if edbm::with_bm(cx, me, |bm| bm.selected_counts().0).unwrap_or(0) < 2 {
        return Ok(0);
    }
    let bm = cx.bmesh_mut(me)?;
    let verts = edbm::sel_verts(bm);
    let verts_len = verts.len();
    let mut is_pair = verts_len == 2;
    let mut check_degenerate = true;
    if pair_share_visible_face(bm, verts[0], verts[1]) {
        check_degenerate = false;
        is_pair = false;
    }
    let backup = bm.clone();
    let r = if is_pair {
        connect_vert_pair(
            bm,
            &ConnectVertPairIn {
                verts,
                verts_exclude: hidden_verts(bm),
                faces_exclude: hidden_faces(bm),
            },
        )
        .map(|o| o.edges)
    } else {
        connect_verts(
            bm,
            &ConnectVertsIn {
                verts,
                faces_exclude: hidden_faces(bm),
                check_degenerate,
            },
        )
        .map(|o| o.edges)
    };
    match r {
        Ok(edges) => {
            let len = edges.len();
            if len > 0 && is_pair {
                let el: Vec<BMElem> = edges.iter().map(|&e| BMElem::Edge(e)).collect();
                edbm::select_enable(bm, &el, true);
            }
            bm.select_flush_mode();
            Ok(len)
        }
        Err(_) => {
            // `BMO_ERROR_FATAL`: `EDBM_redo_state_restore_and_free`.
            *bm = backup;
            Ok(0)
        }
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::VertConnect) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let mut failed = 0usize;
    for &(_, me) in &meshes {
        if connect_vert_pair_mesh(cx, me)? == 0 {
            failed += 1;
        }
    }
    Ok(if failed == meshes.len() {
        OpStatus::Cancelled
    } else {
        OpStatus::Finished
    })
}
