// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.set_normals_from_faces(keep_sharp=False)`, tier T1
//! (`tests/golden/cases/edit/normals/custom_*.py`). Port of
//! `edbm_set_normals_from_faces_exec` (`editors/mesh/editmesh_tools.cc`) and
//! `BM_vert_calc_normal_ex` / `bm_loop_normal_accum` (`bmesh/intern/bmesh_polygon.cc`),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data) with a selected face, each vertex gets the
//! corner-angle-weighted average normal of its **selected** faces, written as the custom
//! normal (`custom_normal`, created when missing) of the corners at that vertex: every corner
//! of every face whose edge at the vertex qualifies (all edges; with `keep_sharp` only
//! selected smooth edges), then every corner sharing a smooth fan with one of those. A vertex
//! without selected faces keeps its normals. The operator always finishes.

use frap_bmesh::{BMesh, ElemFlag, VertId};
use frap_geom::normals::{V3, dot, normalize_v3, safe_acos_approx, sub};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{clnor, edit_meshes_where, sel};

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc (BM_vert_calc_normal_ex).
/// The normal of `v` over its faces with `SELECT`; zero without any.
fn vert_calc_normal_selected(bm: &BMesh, v: VertId) -> V3 {
    let mut no = [0.0f32; 3];
    let mut len = 0usize;
    for l in bm.vert_loops(v) {
        let ll = bm.lp(l);
        if !bm.face(ll.f).flag.contains(ElemFlag::SELECT) {
            continue;
        }
        let co = bm.vert(ll.v).co;
        let v1 = normalize_v3(sub(co, bm.vert(bm.lp(ll.prev).v).co)).0;
        let v2 = normalize_v3(sub(bm.vert(bm.lp(ll.next).v).co, co)).0;
        let fac = safe_acos_approx(-dot(v1, v2));
        let fno = bm.face(ll.f).no;
        no[0] += fno[0] * fac;
        no[1] += fno[1] * fac;
        no[2] += fno[2] * fac;
        len += 1;
    }
    if len > 0 {
        no = normalize_v3(no).0;
    }
    no
}

fn set_normals_from_faces(bm: &mut BMesh, keep_sharp: bool) {
    let sp = clnor::update(bm);
    let verts = bm.vert_ids();
    let mut vert_normals = vec![[0.0f32; 3]; bm.verts.capacity()];
    for &v in &verts {
        vert_normals[v.slot()] = vert_calc_normal_selected(bm, v);
    }
    let mut loop_set = vec![false; bm.loops.capacity()];
    for f in bm.face_ids() {
        for e in bm.face_edges(f) {
            let ef = bm.edge(e).flag;
            if keep_sharp && !(ef.contains(ElemFlag::SMOOTH) && ef.contains(ElemFlag::SELECT)) {
                continue;
            }
            for v in bm.edge(e).v {
                let Some(l) = bm.face_vert_share_loop(f, v) else {
                    continue;
                };
                let vn = vert_normals[v.slot()];
                if vn == [0.0; 3] {
                    continue;
                }
                let data = sp.encode(l, vn);
                clnor::set(bm, l, data);
                if let Some(s) = sp.of(l) {
                    if s.single {
                        loop_set[l.slot()] = true;
                    } else {
                        for &lf in &s.loops {
                            loop_set[lf.slot()] = true;
                        }
                    }
                }
            }
        }
    }
    for &v in &verts {
        for l in bm.vert_loops(v) {
            if loop_set[l.slot()] {
                let data = sp.encode(l, vert_normals[v.slot()]);
                clnor::set(bm, l, data);
            }
        }
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SetNormalsFromFaces) -> OpResult {
    let keep_sharp = p.keep_sharp.value;
    for me in edit_meshes_where(cx, |bm| sel(bm).2 > 0) {
        set_normals_from_faces(cx.bmesh_mut(me)?, keep_sharp);
    }
    Ok(OpStatus::Finished)
}
