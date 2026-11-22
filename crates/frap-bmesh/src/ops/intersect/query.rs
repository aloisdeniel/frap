// SPDX-License-Identifier: GPL-3.0-or-later
//! The BMesh queries `BM_mesh_intersect` calls that the kernel does not expose.
//!
// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_vert_pair_share_face_check,
// BM_face_share_edge_check), bmesh_core.cc (BM_face_pair_overlap_check_subset_same_winding,
// BM_face_pair_overlap_check_subset_swap_winding, BM_vert_splice_check_double_edge,
// BM_vert_splice_check_double_face, BM_vert_collapse_check_double_face) and bmesh_polygon.cc
// (BM_face_calc_tessellation, BM_face_calc_point_in_face), Blender 5.2.4, GPL-2.0-or-later.

use rustc_hash::FxHashSet;

use super::math::V3;
use super::math::mid_v3_v3v3v3;
use crate::types::{BMesh, FaceId, LoopId, VertId};

/// `BM_vert_in_face`.
fn vert_in_face(bm: &BMesh, v: VertId, f: FaceId) -> bool {
    bm.face_loops(f).into_iter().any(|l| bm.lp(l).v == v)
}

/// `BM_vert_pair_share_face_check`.
pub fn vert_pair_share_face_check(bm: &BMesh, v_a: VertId, v_b: VertId) -> bool {
    if bm.vert(v_a).e.is_some() && bm.vert(v_b).e.is_some() {
        return bm
            .vert_faces(v_a)
            .into_iter()
            .any(|f| vert_in_face(bm, v_b, f));
    }
    false
}

/// `BM_face_share_edge_check`.
pub fn face_share_edge_check(bm: &BMesh, f_a: FaceId, f_b: FaceId) -> bool {
    bm.face_loops(f_a)
        .into_iter()
        .any(|l| bm.edge_in_face(bm.lp(l).e, f_b))
}

/// `BM_face_pair_overlap_check_subset_same_winding(l_a, l_a_end, l_b, l_b_end)`.
fn overlap_subset_same_winding(bm: &BMesh, l_a: LoopId, l_a_end: LoopId, l_b: LoopId) -> bool {
    let (mut a, mut b) = (l_a, l_b);
    loop {
        if bm.lp(b).v != bm.lp(a).v {
            return false;
        }
        if a == l_a_end {
            return true;
        }
        a = bm.lp(a).next;
        b = bm.lp(b).next;
    }
}

/// `BM_face_pair_overlap_check_subset_swap_winding(l_a, l_a_end, l_b, l_b_end)`.
fn overlap_subset_swap_winding(bm: &BMesh, l_a: LoopId, l_a_end: LoopId, l_b: LoopId) -> bool {
    let (mut a, mut b) = (l_a, l_b);
    loop {
        if bm.lp(b).v != bm.lp(a).v {
            return false;
        }
        if a == l_a_end {
            return true;
        }
        a = bm.lp(a).next;
        b = bm.lp(b).prev;
    }
}

/// `BM_vert_splice_check_double_edge(v_a, v_b)`: splicing would make two edges between the
/// same vertices (a neighbour of `v_a` is a neighbour of `v_b`).
pub fn vert_splice_check_double_edge(bm: &BMesh, v_a: VertId, v_b: VertId) -> bool {
    if bm.vert(v_a).e.is_none() || bm.vert(v_b).e.is_none() {
        return false;
    }
    let visit: FxHashSet<VertId> = bm
        .vert_edges(v_a)
        .into_iter()
        .map(|e| bm.edge_other_vert(e, v_a))
        .collect();
    bm.vert_edges(v_b)
        .into_iter()
        .any(|e| visit.contains(&bm.edge_other_vert(e, v_b)))
}

/// `BM_vert_splice_check_double_face(v_a, v_b)`: splicing would make two faces over the
/// same vertices.
pub fn vert_splice_check_double_face(bm: &BMesh, v_a: VertId, v_b: VertId) -> bool {
    if bm.vert(v_a).e.is_none() || bm.vert(v_b).e.is_none() {
        return false;
    }
    let v_pair = [v_a, v_b];
    let mut loops_pair: [Vec<LoopId>; 2] = [Vec::new(), Vec::new()];
    for side in 0..2 {
        let v = v_pair[side];
        for e in bm.vert_edges(v) {
            for l in bm.edge_loops(e) {
                if bm.lp(l).v == v {
                    loops_pair[side].push(l);
                }
            }
        }
        if loops_pair[side].is_empty() {
            return false;
        }
    }
    let flen = |l: LoopId| bm.face(bm.lp(l).f).len;
    for lp in &mut loops_pair {
        if lp.len() > 1 {
            // `std::ranges::sort` by face length (not stable, but only the grouping by length
            // matters below).
            lp.sort_by_key(|&l| flen(l));
        }
    }
    let (la, lb) = (&loops_pair[0], &loops_pair[1]);
    let (mut a_beg, mut b_beg) = (0usize, 0usize);
    while a_beg < la.len() && b_beg < lb.len() {
        let a_len = flen(la[a_beg]);
        let b_len = flen(lb[b_beg]);
        if a_len < b_len {
            a_beg += 1;
            continue;
        }
        if a_len > b_len {
            b_beg += 1;
            continue;
        }
        let f_len = a_len;
        let mut a_end = a_beg + 1;
        let mut b_end = b_beg + 1;
        while a_end < la.len() && flen(la[a_end]) == f_len {
            a_end += 1;
        }
        while b_end < lb.len() && flen(lb[b_end]) == f_len {
            b_end += 1;
        }
        for &l_a in &la[a_beg..a_end] {
            for &l_b in &lb[b_beg..b_end] {
                let (a, b) = (bm.lp(l_a), bm.lp(l_b));
                let v = |l: LoopId| bm.lp(l).v;
                if v(a.next) == v(b.next) && v(a.prev) == v(b.prev) {
                    if overlap_subset_same_winding(
                        bm,
                        bm.lp(a.next).next,
                        bm.lp(a.prev).prev,
                        bm.lp(b.next).next,
                    ) {
                        return true;
                    }
                } else if v(a.next) == v(b.prev)
                    && v(a.prev) == v(b.next)
                    && overlap_subset_swap_winding(
                        bm,
                        bm.lp(a.next).next,
                        bm.lp(a.prev).prev,
                        bm.lp(b.prev).prev,
                    )
                {
                    return true;
                }
            }
        }
        a_beg = a_end;
        b_beg = b_end;
    }
    false
}

/// `BM_vert_collapse_check_double_face(v_collapse)`: collapsing the edge-pair vertex
/// `v_collapse` would make one of its faces a duplicate of a face on the edge it creates.
pub fn vert_collapse_check_double_face(bm: &BMesh, v_collapse: VertId) -> bool {
    let Some(e_a) = bm.vert(v_collapse).e else {
        return false;
    };
    let Some(l_collapse_a_first) = bm.edge(e_a).l else {
        return false;
    };
    let lf = bm.lp(l_collapse_a_first);
    let e_b = if lf.v == v_collapse {
        bm.lp(lf.prev).e
    } else {
        bm.lp(lf.next).e
    };
    let v_a = bm.edge_other_vert(e_a, v_collapse);
    let v_b = bm.edge_other_vert(e_b, v_collapse);
    let Some(e_exists) = bm.edge_exists(v_a, v_b) else {
        return false;
    };
    let Some(l_exists_first) = bm.edge(e_exists).l else {
        return false;
    };
    let mut l_collapse_a_iter = l_collapse_a_first;
    loop {
        let it = bm.lp(l_collapse_a_iter);
        let it_len = bm.face(it.f).len;
        if it_len > 3 {
            let l_collapse = if it.v == v_collapse {
                l_collapse_a_iter
            } else if bm.lp(it.next).v == v_collapse {
                it.next
            } else {
                it.prev
            };
            let mut l_exists_iter = l_exists_first;
            loop {
                let ex = bm.lp(l_exists_iter);
                if it_len - 1 == bm.face(ex.f).len {
                    let l_collapse_beg = bm.lp(l_collapse).next;
                    let l_collapse_end = bm.lp(l_collapse).prev;
                    let swap_winding = bm.lp(l_collapse_beg).v == ex.v;
                    let (l_exists_beg, l_exists_end) = if swap_winding {
                        (l_exists_iter, ex.next)
                    } else {
                        (ex.next, l_exists_iter)
                    };
                    let _ = l_exists_end;
                    let found = if swap_winding {
                        overlap_subset_swap_winding(
                            bm,
                            bm.lp(l_collapse_beg).next,
                            bm.lp(l_collapse_end).prev,
                            bm.lp(l_exists_beg).prev,
                        )
                    } else {
                        overlap_subset_same_winding(
                            bm,
                            bm.lp(l_collapse_beg).next,
                            bm.lp(l_collapse_end).prev,
                            bm.lp(l_exists_beg).next,
                        )
                    };
                    if found {
                        return true;
                    }
                }
                l_exists_iter = ex.radial_next;
                if l_exists_iter == l_exists_first {
                    break;
                }
            }
        }
        l_collapse_a_iter = it.radial_next;
        if l_collapse_a_iter == l_collapse_a_first {
            break;
        }
    }
    false
}

/// `BM_face_calc_tessellation(f, use_fixed_quad = false, r_loops, r_index)`: the face's
/// loops and triangles (indices into them); quads go through the polyfill too.
pub fn face_calc_tessellation(bm: &BMesh, f: FaceId) -> (Vec<LoopId>, Vec<[u32; 3]>) {
    let loops = bm.face_loops(f);
    if loops.len() == 3 {
        return (loops, vec![[0, 1, 2]]);
    }
    let m = frap_geom::polyfill::axis_dominant_v3_to_m3_negate(bm.face(f).no);
    let proj: Vec<[f32; 2]> = loops
        .iter()
        .map(|&l| {
            let a = bm.vert(bm.lp(l).v).co;
            // `mul_v2_m3v3` with the column-major matrix.
            [
                m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
                m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
            ]
        })
        .collect();
    let tris = frap_geom::polyfill::polyfill_calc(&proj, 1);
    (loops, tris)
}

/// `BM_face_calc_point_in_face`: the centre of the face's largest tessellation triangle (a
/// point inside even a concave face).
pub fn face_calc_point_in_face(bm: &BMesh, f: FaceId) -> V3 {
    let co = |l: LoopId| bm.vert(bm.lp(l).v).co;
    let ltri: [LoopId; 3] = if bm.face(f).len == 3 {
        let l = bm.face(f).l_first;
        [l, bm.lp(l).next, bm.lp(l).prev]
    } else {
        let (loops, index) = face_calc_tessellation(bm, f);
        let mut j_best = 0usize;
        let mut area_best = -1.0f32;
        for (j, t) in index.iter().enumerate() {
            let area = super::math::area_squared_tri_v3(
                co(loops[t[0] as usize]),
                co(loops[t[1] as usize]),
                co(loops[t[2] as usize]),
            );
            if area > area_best {
                j_best = j;
                area_best = area;
            }
        }
        let t = index[j_best];
        [
            loops[t[0] as usize],
            loops[t[1] as usize],
            loops[t[2] as usize],
        ]
    };
    mid_v3_v3v3v3(co(ltri[0]), co(ltri[1]), co(ltri[2]))
}
