// SPDX-License-Identifier: GPL-3.0-or-later
//! The BMesh tessellations `BM_mesh_intersect`'s callers hand it: Edit Mode's `em->looptris`
//! (`BM_mesh_calc_tessellation`) and the BOOLEAN modifier's `BM_mesh_calc_tessellation_beauty`.
//!
// Ported from blender/source/blender/bmesh/intern/bmesh_mesh_tessellate.cc
// (bmesh_calc_tessellation_for_face_impl, BM_mesh_calc_tessellation,
// bmesh_calc_tessellation_for_face_beauty, BM_mesh_calc_tessellation_beauty), Blender 5.2.4,
// GPL-2.0-or-later. Faces are visited in iteration order and their triangles appended, as the
// single-threaded path does (the threaded one writes the same triangles at the same offsets).

use crate::ops::contextual_create::bli;
use crate::ops::triangulate::beautify::{polyfill_beautify, quad_rotate_calc_ex};
use crate::query::geom::V3;
use crate::types::{BMesh, FaceId, LoopId};

/// `mul_v2_m3v3` with a matrix from `axis_dominant_v3_to_m3_negate` (column-major).
fn project_negate(m: &[V3; 3], a: V3) -> [f32; 2] {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
    ]
}

/// The polyfill of an n-gon (`axis_dominant_v3_to_m3_negate(f->no)`, `BLI_polyfill_calc`),
/// optionally beautified: the loops and the triangles over them.
fn ngon_tris(bm: &BMesh, f: FaceId, beauty: bool) -> (Vec<LoopId>, Vec<[u32; 3]>) {
    let loops = bm.face_loops(f);
    let m = frap_geom::polyfill::axis_dominant_v3_to_m3_negate(bm.face(f).no);
    let proj: Vec<[f32; 2]> = loops
        .iter()
        .map(|&l| project_negate(&m, bm.vert(bm.lp(l).v).co))
        .collect();
    let mut tris = frap_geom::polyfill::polyfill_calc(&proj, 1);
    if beauty {
        polyfill_beautify(&proj, &mut tris);
    }
    (loops, tris)
}

/// `BM_mesh_calc_tessellation(bm, looptris)` (stored face normals, as `em->looptris` after
/// `EDBM_update`): quads split on 0–2 unless that is degenerate
/// (`is_quad_flip_v3_first_third_fast`), n-gons by polyfill in the plane of the face normal.
pub fn mesh_calc_tessellation(bm: &BMesh) -> Vec<[LoopId; 3]> {
    let mut out = Vec::with_capacity(bm.loops_num());
    for f in bm.face_ids() {
        frap_math::budget::check();
        let len = bm.face(f).len;
        match len {
            0..=2 => {}
            3 => {
                let l0 = bm.face(f).l_first;
                let l1 = bm.lp(l0).next;
                out.push([l0, l1, bm.lp(l1).next]);
            }
            4 => {
                let l0 = bm.face(f).l_first;
                let l1 = bm.lp(l0).next;
                let l2 = bm.lp(l1).next;
                let l3 = bm.lp(l2).next;
                let co = |l: LoopId| bm.vert(bm.lp(l).v).co;
                let mut a = [l0, l1, l2];
                let mut b = [l0, l2, l3];
                if frap_geom::polyfill::is_quad_flip_v3_first_third_fast(
                    co(l0),
                    co(l1),
                    co(l2),
                    co(l3),
                ) {
                    // Flip out of the degenerate 0-2 state.
                    a[2] = b[2];
                    b[0] = a[1];
                }
                out.push(a);
                out.push(b);
            }
            _ => {
                let (loops, tris) = ngon_tris(bm, f, false);
                for t in tris {
                    out.push(t.map(|i| loops[i as usize]));
                }
            }
        }
    }
    out
}

/// `BM_mesh_calc_tessellation_beauty(bm, looptris)`: quads split on the diagonal
/// `BLI_polyfill_beautify_quad_rotate_calc` prefers in the plane of the face normal, n-gons by
/// polyfill + `BLI_polyfill_beautify`.
pub fn mesh_calc_tessellation_beauty(bm: &BMesh) -> Vec<[LoopId; 3]> {
    let mut out = Vec::with_capacity(bm.loops_num());
    for f in bm.face_ids() {
        frap_math::budget::check();
        let len = bm.face(f).len;
        match len {
            0..=2 => {}
            3 => {
                let l0 = bm.face(f).l_first;
                let l1 = bm.lp(l0).next;
                out.push([l0, l1, bm.lp(l1).next]);
            }
            4 => {
                let l_v1 = bm.face(f).l_first;
                let l_v2 = bm.lp(l_v1).next;
                let l_v3 = bm.lp(l_v2).next;
                let l_v4 = bm.lp(l_v1).prev;
                let m = bli::axis_dominant_v3_to_m3(bm.face(f).no);
                let p = |l: LoopId| bli::mul_v2_m3v3(&m, bm.vert(bm.lp(l).v).co);
                let split_13 =
                    quad_rotate_calc_ex(p(l_v1), p(l_v2), p(l_v3), p(l_v4), false).0 < 0.0;
                if split_13 {
                    out.push([l_v1, l_v2, l_v3]);
                    out.push([l_v1, l_v3, l_v4]);
                } else {
                    out.push([l_v1, l_v2, l_v4]);
                    out.push([l_v2, l_v3, l_v4]);
                }
            }
            _ => {
                let (loops, tris) = ngon_tris(bm, f, true);
                for t in tris {
                    out.push(t.map(|i| loops[i as usize]));
                }
            }
        }
    }
    out
}
