// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_overlap(extend=False)`, tier T1
//! (`tests/golden/cases/uv/select/overlap*.py`). Port of `uv_select_overlap` and
//! `overlap_tri_tri_uv_test` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. Unless `extend`, every UV is deselected first. Every visible
//! face of every mesh in Edit Mode with a UV map is triangulated in UV space (`polyfill` +
//! `beautify`); the faces with a triangle overlapping a triangle of another face (of any of
//! the meshes) are selected with the sticky mode, with `use_uv_select_island` their whole UV
//! islands. Touching corners or edges are no overlap; identical triangles are. Always finishes.
//!
//! Blender walks a BVH of the triangles and stops each face's query at its first overlap; the
//! set of faces it tags is every face with an overlapping partner, which this port finds by
//! testing every pair whose bounds touch.

mod beautify;
mod heap;

use frap_bmesh::{ElemFlag, FaceId};
use frap_bmesh::ops::contextual_create::connect_islands::isect_seg_seg_v2_point_ex;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::linked::LinkedHelper;
use crate::select::{SelAction, Uv, UvSel, op};

/// One UV triangle of a face.
struct Tri {
    ob: usize,
    face: FaceId,
    co: [Uv; 3],
    min: Uv,
    max: Uv,
}

/// `line_point_side_v2`.
fn line_point_side_v2(l1: Uv, l2: Uv, pt: Uv) -> f32 {
    ((l1[0] - pt[0]) * (l2[1] - pt[1])) - ((l2[0] - pt[0]) * (l1[1] - pt[1]))
}

/// `isect_point_tri_v2 != 0`.
fn isect_point_tri_v2(pt: Uv, v1: Uv, v2: Uv, v3: Uv) -> bool {
    let s12 = line_point_side_v2(v1, v2, pt);
    let s23 = line_point_side_v2(v2, v3, pt);
    let s31 = line_point_side_v2(v3, v1, pt);
    (s12 >= 0.0 && s23 >= 0.0 && s31 >= 0.0) || (s12 <= 0.0 && s23 <= 0.0 && s31 <= 0.0)
}

/// `mid_v2_v2v2v2`.
fn mid3(t: &[Uv; 3]) -> Uv {
    [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
    ]
}

/// `overlap_tri_tri_uv_test`.
pub(crate) fn overlap_tri_tri_uv_test(t1: &[Uv; 3], t2: &[Uv; 3], endpoint_bias: f32) -> bool {
    for (a, b) in [(0, 1), (1, 2), (2, 0)] {
        for (c, d) in [(0, 1), (1, 2), (2, 0)] {
            // Blender tests 8 of the 9 pairs: not (t1[2], t1[0]) × (t2[2], t2[0]).
            if a == 2 && c == 2 {
                continue;
            }
            if isect_seg_seg_v2_point_ex(t1[a], t1[b], t2[c], t2[d], endpoint_bias).is_some() {
                return true;
            }
        }
    }
    if isect_point_tri_v2(mid3(t1), t2[0], t2[1], t2[2]) {
        return true;
    }
    isect_point_tri_v2(mid3(t2), t1[0], t1[1], t1[2])
}

/// The triangles of the visible faces of one mesh.
fn triangles(s: &UvSel, ob: usize, out: &mut Vec<Tri>) {
    for f in s.faces() {
        if !s.face_visible(f) {
            continue;
        }
        let coords: Vec<Uv> = s.face_loops(f).into_iter().map(|l| s.uv(l)).collect();
        let mut tris = frap_geom::polyfill::polyfill_calc(&coords, 0);
        beautify::polyfill_beautify(&coords, &mut tris);
        for t in tris {
            let co = [
                coords[t[0] as usize],
                coords[t[1] as usize],
                coords[t[2] as usize],
            ];
            let mut min = co[0];
            let mut max = co[0];
            for c in &co[1..] {
                for k in 0..2 {
                    min[k] = min[k].min(c[k]);
                    max[k] = max[k].max(c[k]);
                }
            }
            out.push(Tri {
                ob,
                face: f,
                co,
                min,
                max,
            });
        }
    }
}

/// The faces (by object) with a triangle overlapping another face's.
fn overlapping(tris: &[Tri], objects: usize) -> Vec<Vec<FaceId>> {
    let mut hit = vec![false; tris.len()];
    for i in 0..tris.len() {
        frap_math::budget::check();
        let a = &tris[i];
        for (j, b) in tris.iter().enumerate().skip(i + 1) {
            if a.ob == b.ob && a.face == b.face {
                continue;
            }
            let bounds = a.min[0] <= b.max[0]
                && a.max[0] >= b.min[0]
                && a.min[1] <= b.max[1]
                && a.max[1] >= b.min[1];
            if bounds && overlap_tri_tri_uv_test(&a.co, &b.co, -1e-4) {
                hit[i] = true;
                hit[j] = true;
            }
        }
    }
    let mut out = vec![Vec::new(); objects];
    for (t, h) in tris.iter().zip(hit) {
        if h && !out[t.ob].contains(&t.face) {
            out[t.ob].push(t.face);
        }
    }
    out
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SelectOverlap) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let extend = p.extend.value;
    let select_island = ts.select_island_check();
    let objects = op::objects(cx);
    let mut tris = Vec::new();
    for (i, &(_, me)) in objects.iter().enumerate() {
        op::read(cx, me, ts, |s| triangles(s, i, &mut tris));
    }
    let tagged = overlapping(&tris, objects.len());
    for (i, &(_, me)) in objects.iter().enumerate() {
        let faces = &tagged[i];
        op::edit(cx, me, ts, |s| {
            s.face_tags_clear();
            if !extend {
                s.deselect_all(SelAction::Deselect);
            }
            if faces.is_empty() {
                return;
            }
            for &f in faces {
                s.set_fflag(f, ElemFlag::TAG, true);
            }
            if select_island {
                let mut helper = LinkedHelper::new();
                for f in s.faces() {
                    if s.fflag(f, ElemFlag::TAG) {
                        helper.face_add(s, f);
                    }
                }
                helper.tag_all(s);
            }
            s.prepare();
            s.flush_from_tag_face(true);
            if s.ts.sync {
                s.select_sync_flush(true);
            } else {
                s.selectmode_flush();
            }
        })?;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_is_no_overlap_identical_is() {
        let a = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let b = [[1.0, 0.0], [2.0, 0.0], [1.0, 1.0]];
        let c = [[0.2, 0.2], [1.2, 0.2], [0.2, 1.2]];
        assert!(!overlap_tri_tri_uv_test(&a, &b, -1e-4));
        assert!(overlap_tri_tri_uv_test(&a, &a, -1e-4));
        assert!(overlap_tri_tri_uv_test(&a, &c, -1e-4));
    }
}
