// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.convex_hull(delete_unused=True, use_existing_faces=True, make_holes=False,
//! join_triangles=True, face_threshold=40°, shape_threshold=40°, …)`, tier T1
//! (`tests/golden/cases/edit/create/convex_hull*.py`). Port of `edbm_convex_hull_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `bmesh.ops.convex_hull`.
//!
//! The hull of the selected vertices is built and selected; unused interior geometry is
//! deleted (`delete_unused`), hull faces replacing selected faces are removed (`make_holes`),
//! and coplanar hull triangles are merged into quads (`join_triangles`, WP 2.10's
//! `join_triangles` op). Fewer than three vertices: INFO `Requires at least three vertices`.

use frap_bmesh::ops::convex_hull::{ConvexHullIn, convex_hull};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::ConvexHull) -> OpResult {
    run(cx, p)
}

fn run(cx: &mut OpCtx, p: mesh::ConvexHull) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().0) == Some(0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let input = edbm::sel_geom(bm);
        let r = convex_hull(
            bm,
            &ConvexHullIn {
                input,
                use_existing_faces: p.use_existing_faces.value,
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::faces_of(&out.geom);
        edbm::select_enable(bm, &faces, true);
        if p.delete_unused.value {
            let geom: Vec<_> = out
                .geom_unused
                .iter()
                .copied()
                .filter(|g| bm.elem_alive(*g))
                .collect();
            let r = frap_bmesh::ops::delete::delete(
                bm,
                &frap_bmesh::ops::delete::DeleteIn {
                    geom,
                    context: "TAGGED_ONLY",
                },
            );
            if edbm::finish(cx, r)?.is_none() {
                continue;
            }
        }
        let bm = cx.bmesh_mut(me)?;
        if p.make_holes.value {
            let geom: Vec<_> = out
                .geom_holes
                .iter()
                .copied()
                .filter(|g| bm.elem_alive(*g))
                .collect();
            let r = frap_bmesh::ops::delete::delete(
                bm,
                &frap_bmesh::ops::delete::DeleteIn {
                    geom,
                    context: "TAGGED_ONLY",
                },
            );
            if edbm::finish(cx, r)?.is_none() {
                continue;
            }
        }
        let bm = cx.bmesh_mut(me)?;
        if p.join_triangles.value {
            let faces: Vec<_> = out
                .geom
                .iter()
                .filter_map(|g| match g {
                    frap_bmesh::BMElem::Face(f) if bm.faces.contains(f.0) => Some(*f),
                    _ => None,
                })
                .collect();
            let r = frap_bmesh::ops::join_triangles::join_triangles(
                bm,
                &frap_bmesh::ops::join_triangles::JoinTrianglesIn {
                    faces,
                    angle_face_threshold: p.face_threshold.value,
                    angle_shape_threshold: p.shape_threshold.value,
                    ..Default::default()
                },
            );
            let Some(j) = edbm::finish(cx, r)? else {
                continue;
            };
            let bm = cx.bmesh_mut(me)?;
            let faces = edbm::faces_of(&j.geom);
            edbm::select_enable(bm, &faces, true);
        }
        let bm = cx.bmesh_mut(me)?;
        bm.select_flush_mode();
    }
    Ok(OpStatus::Finished)
}
