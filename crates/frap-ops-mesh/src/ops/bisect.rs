// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.bisect(plane_co, plane_no, use_fill=False, clear_inner=False,
//! clear_outer=False, threshold=0.0001)`, tier T2
//! (`tests/golden/cases/edit/create/bisect_*.py`). Port of `mesh_bisect_exec`
//! (`editors/mesh/editmesh_bisect.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `bmesh.ops.bisect_plane`; the interactive line drawing is not available headless (pass the
//! plane: `plane_co` defaults to the 3D cursor, `plane_no` to the view's up axis).
//!
//! Every mesh in Edit Mode with selected edges is cut by the plane given in world space; the
//! selection becomes the cut (vertices and edges on the plane). `use_fill` fills the cut with
//! `triangle_fill(use_dissolve)` and takes attributes from the neighbouring faces; the fill
//! faces are selected too. `clear_inner`/`clear_outer` remove the selected geometry behind or
//! in front of the plane. `CANCELLED` when no mesh had a selection.

use frap_bmesh::ops::bisect_plane::{BisectPlaneIn, bisect_plane};
use frap_bmesh::ops::contextual_create::{attribute_fill, bli};
use frap_bmesh::ops::triangle_fill::{TriangleFillIn, triangle_fill};
use frap_bmesh::{BMElem, EdgeId, FaceId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

/// `mul_m4_v3`: `m * (v, 1)` with Blender's column-major layout.
fn mul_m4_v3(m: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    [
        v[0] * m[0][0] + v[1] * m[1][0] + m[2][0] * v[2] + m[3][0],
        v[0] * m[0][1] + v[1] * m[1][1] + m[2][1] * v[2] + m[3][1],
        v[0] * m[0][2] + v[1] * m[1][2] + m[2][2] * v[2] + m[3][2],
    ]
}

/// `mul_transposed_mat3_m4_v3`.
fn mul_transposed_mat3_m4_v3(m: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    [
        v[0] * m[0][0] + v[1] * m[0][1] + v[2] * m[0][2],
        v[0] * m[1][0] + v[1] * m[1][1] + v[2] * m[1][2],
        v[0] * m[2][0] + v[1] * m[2][1] + v[2] * m[2][2],
    ]
}

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::Bisect) -> OpResult {
    let thresh = p.threshold.value;
    let use_fill = p.use_fill.value;
    let clear_inner = p.clear_inner.value;
    let clear_outer = p.clear_outer.value;
    let (plane_co, plane_no) = {
        let c = cx.ctx();
        let co = if p.plane_co.is_set {
            p.plane_co.value
        } else {
            c.scene().cursor.location
        };
        let no = if p.plane_no.is_set {
            p.plane_no.value
        } else {
            // `rv3d->viewinv[1]`: the view's up axis in world space.
            let vinv = c.win.view3d.view_matrix.inverse();
            vinv.y_axis.truncate().to_array()
        };
        (co, no)
    };
    let mut ret = OpStatus::Cancelled;
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().1) == Some(0) {
            continue;
        }
        let obmat = frap_scene::id::transform::world_matrix(&cx.main().objects, ob);
        let m = obmat.to_cols_array_2d();
        let imat = obmat.inverse().to_cols_array_2d();
        let co_local = mul_m4_v3(&imat, plane_co);
        let no_local = mul_transposed_mat3_m4_v3(&m, plane_no);
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::sel_geom(bm);
        let r = bisect_plane(
            bm,
            &BisectPlaneIn {
                geom,
                dist: thresh,
                plane_co: co_local,
                plane_no: no_local,
                clear_inner,
                clear_outer,
                ..Default::default()
            },
        );
        let bm = cx.bmesh_mut(me)?;
        edbm::deselect_all(bm);
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        if use_fill {
            let normal_fill = bli::normalize_v3(no_local).0;
            let edges: Vec<EdgeId> = out
                .geom_cut
                .iter()
                .filter_map(|g| match g {
                    BMElem::Edge(e) => Some(*e),
                    _ => None,
                })
                .collect();
            if let Ok(fill) = triangle_fill(
                bm,
                &TriangleFillIn {
                    edges,
                    normal: normal_fill,
                    use_dissolve: true,
                    ..Default::default()
                },
            ) {
                let faces: Vec<FaceId> = fill
                    .geom
                    .iter()
                    .filter_map(|g| match g {
                        BMElem::Face(f) => Some(*f),
                        _ => None,
                    })
                    .filter(|f| bm.faces.contains(f.0))
                    .collect();
                // `face_attribute_fill` as a sub-operator (no recalculation of lone faces).
                let caps = [
                    bm.verts.capacity(),
                    bm.edges.capacity(),
                    bm.faces.capacity(),
                ];
                bm.oflags.push(caps);
                attribute_fill::face_attribute_fill(bm, &faces, true, true);
                bm.oflags.pop();
                let fe: Vec<BMElem> = faces.iter().map(|&f| BMElem::Face(f)).collect();
                edbm::select_enable(bm, &fe, true);
            }
        }
        let cut: Vec<BMElem> = out
            .geom_cut
            .iter()
            .copied()
            .filter(|g| bm.elem_alive(*g))
            .collect();
        edbm::select_enable(bm, &cut, true);
        bm.select_flush_mode();
        ret = OpStatus::Finished;
    }
    Ok(ret)
}
