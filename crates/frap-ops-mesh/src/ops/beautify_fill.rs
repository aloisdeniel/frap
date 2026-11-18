// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.beautify_fill(angle_limit=π)`, tier T1
//! (`tests/golden/cases/edit/create/beautify_fill.py`). Port of `edbm_beautify_fill_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `bmesh.ops.beautify_fill`: edges between selected triangles are rotated while the shape
//! improves (AREA method); with `angle_limit < π` only edges whose faces meet flatter than
//! the limit may rotate. The rotated edges and their faces are added to the selection.

use frap_bmesh::ops::beautify_fill::{BeautifyFillIn, beautify_fill};
use frap_bmesh::ops::contextual_create::bli;
use frap_bmesh::{BMesh, EdgeId, ElemFlag};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

/// `BM_edge_calc_face_angle_ex(e, fallback)`: the angle between the two face normals of a
/// manifold edge.
fn edge_calc_face_angle_ex(bm: &BMesh, e: EdgeId, fallback: f32) -> f32 {
    let Some(l) = bm.edge(e).l else {
        return fallback;
    };
    let lr = bm.lp(l).radial_next;
    if lr == l || bm.lp(lr).radial_next != l {
        return fallback;
    }
    let (na, nb) = (bm.face(bm.lp(l).f).no, bm.face(bm.lp(lr).f).no);
    bli::angle_normalized_v3v3(na, nb)
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::BeautifyFill) -> OpResult {
    let angle_max = bli::PI;
    let angle_limit = p.angle_limit.value;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().2) == Some(0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let flag = if angle_limit >= angle_max {
            ElemFlag::SELECT
        } else {
            for e in bm.edge_ids() {
                let t = bm.edge(e).flag.contains(ElemFlag::SELECT)
                    && edge_calc_face_angle_ex(bm, e, angle_max) < angle_limit;
                bm.edge_mut(e).flag.set(ElemFlag::TAG, t);
            }
            ElemFlag::TAG
        };
        let faces = edbm::sel_faces(bm);
        let edges = edbm::edges_with(bm, flag);
        let r = beautify_fill(
            bm,
            &BeautifyFillIn {
                faces,
                edges,
                ..Default::default()
            },
        );
        if let Some(out) = edbm::finish(cx, r)? {
            let bm = cx.bmesh_mut(me)?;
            edbm::select_enable(bm, &out.geom, true);
        }
    }
    Ok(OpStatus::Finished)
}
