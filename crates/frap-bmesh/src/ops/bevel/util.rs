// SPDX-License-Identifier: GPL-3.0-or-later
//! BMesh queries the bevel port needs that the core does not provide, and loop-layer "math"
//! tests. Ported from blender/source/blender/bmesh/intern/{bmesh_query.cc, bmesh_polygon.cc}
//! and blenkernel/intern/customdata.cc (Blender 5.2.4, GPL-2.0-or-later).

use frap_mesh::AttrData;

use super::math::{self, V3};
use crate::query::geom;
use crate::types::{BMesh, EdgeId, FaceId, LoopId, VertId};

/// `v->co`.
#[inline]
pub(crate) fn co(bm: &BMesh, v: VertId) -> V3 {
    bm.vert(v).co
}

/// `f->no`.
#[inline]
pub(crate) fn fno(bm: &BMesh, f: FaceId) -> V3 {
    bm.face(f).no
}

/// `e->v1`, `e->v2`.
#[inline]
pub(crate) fn ev(bm: &BMesh, e: EdgeId) -> [VertId; 2] {
    bm.edge(e).v
}

/// `BM_edge_calc_length`.
pub(crate) fn edge_calc_length(bm: &BMesh, e: EdgeId) -> f32 {
    let [a, b] = ev(bm, e);
    math::len_v3v3(co(bm, a), co(bm, b))
}

/// `BM_vert_face_check`: is any face using `v`?
pub(crate) fn vert_face_check(bm: &BMesh, v: VertId) -> bool {
    bm.vert_edges(v).into_iter().any(|e| bm.edge(e).l.is_some())
}

/// `BM_edge_loop_pair`: the two loops of a manifold edge.
pub(crate) fn edge_loop_pair(bm: &BMesh, e: EdgeId) -> Option<(LoopId, LoopId)> {
    let la = bm.edge(e).l?;
    let lb = bm.lp(la).radial_next;
    if la != lb && bm.lp(lb).radial_next == la {
        Some((la, lb))
    } else {
        None
    }
}

/// `BM_face_calc_center_bounds`.
pub(crate) fn face_calc_center_bounds(bm: &BMesh, f: FaceId) -> V3 {
    let mut min = [f32::MAX; 3];
    let mut max = [-f32::MAX; 3];
    for v in bm.face_verts(f) {
        let c = co(bm, v);
        for i in 0..3 {
            if c[i] < min[i] {
                min[i] = c[i];
            }
            if c[i] > max[i] {
                max[i] = c[i];
            }
        }
    }
    math::mid(min, max)
}

/// `BM_face_point_inside_test`.
pub(crate) fn face_point_inside_test(bm: &BMesh, f: FaceId, p: V3) -> bool {
    let rows = geom::axis_dominant_v3_rows(fno(bm, f));
    let co_2d = geom::project_v2(&rows, p);
    let projverts: Vec<[f32; 2]> = bm
        .face_verts(f)
        .into_iter()
        .map(|v| geom::project_v2(&rows, co(bm, v)))
        .collect();
    math::isect_point_poly_v2(co_2d, &projverts)
}

/// `BM_edge_is_convex`.
fn edge_is_convex(bm: &BMesh, e: EdgeId) -> bool {
    if bm.edge_is_manifold(e) {
        let l1 = bm.edge(e).l.expect("manifold edge");
        let l2 = bm.lp(l1).radial_next;
        let (n1, n2) = (fno(bm, bm.lp(l1).f), fno(bm, bm.lp(l2).f));
        if n1 != n2 {
            let cross = math::cross(n1, n2);
            let l_dir = math::sub(co(bm, bm.lp(bm.lp(l1).next).v), co(bm, bm.lp(l1).v));
            return math::dot(l_dir, cross) > 0.0;
        }
    }
    true
}

/// `BM_edge_calc_face_angle_signed_ex`.
pub(crate) fn edge_calc_face_angle_signed_ex(bm: &BMesh, e: EdgeId, fallback: f32) -> f32 {
    if bm.edge_is_manifold(e) {
        let l1 = bm.edge(e).l.expect("manifold edge");
        let l2 = bm.lp(l1).radial_next;
        let angle = math::angle_normalized_v3v3(fno(bm, bm.lp(l1).f), fno(bm, bm.lp(l2).f));
        return if edge_is_convex(bm, e) { angle } else { -angle };
    }
    fallback
}

/// `CustomData_layer_has_math` on the loop domain (UV maps and colours).
pub(crate) fn layer_has_math(d: &AttrData) -> bool {
    matches!(
        d,
        AttrData::Float2(_) | AttrData::FloatColor(_) | AttrData::ByteColor(_)
    )
}

/// `CustomData_data_equals` for the layers with math (`layerEqual_*`), at two loop slots.
pub(crate) fn layer_equal(d: &AttrData, a: LoopId, b: LoopId) -> bool {
    let (a, b) = (a.slot(), b.slot());
    match d {
        AttrData::Float2(c) => {
            let (p, q) = (c[a], c[b]);
            let (dx, dy) = (p[0] - q[0], p[1] - q[1]);
            dx * dx + dy * dy < 0.00001
        }
        AttrData::FloatColor(c) => {
            let mut tot = 0.0f32;
            for i in 0..4 {
                let x = c[a][i] - c[b][i];
                tot += x * x;
            }
            tot < 0.001
        }
        AttrData::ByteColor(c) => {
            let mut tot = 0.0f32;
            for i in 0..4 {
                let x = (i32::from(c[a][i]) - i32::from(c[b][i])) as f32;
                tot += x * x;
            }
            tot < 0.001
        }
        _ => true,
    }
}

/// `contig_ldata_across_loops` for every loop layer with math.
fn contig_all_layers(bm: &BMesh, l1: LoopId, l2: LoopId) -> bool {
    bm.ldata
        .layers
        .iter()
        .filter(|l| layer_has_math(&l.data))
        .all(|l| layer_equal(&l.data, l1, l2))
}

/// `contig_ldata_across_edge`: are all loop layers with math contiguous from `f1` to `f2`
/// across `e`?
pub(crate) fn contig_ldata_across_edge(bm: &BMesh, e: EdgeId, f1: FaceId, f2: FaceId) -> bool {
    if bm.ldata.layers.is_empty() {
        return true;
    }
    let Some((mut lef1, mut lef2)) = edge_loop_pair(bm, e) else {
        return false;
    };
    if bm.lp(lef1).f == f2 {
        std::mem::swap(&mut lef1, &mut lef2);
    }
    if bm.lp(lef1).f != f1 || bm.lp(lef2).f != f2 {
        return false;
    }
    let v1 = bm.lp(lef1).v;
    let v2 = bm.lp(lef2).v;
    if v1 == v2 {
        return false;
    }
    let lv1f1 = lef1;
    let lv2f1 = bm.lp(lef1).next;
    let lv1f2 = bm.lp(lef2).next;
    let lv2f2 = lef2;
    // Blender checks layer by layer, both corners per layer; the result is the same.
    bm.ldata
        .layers
        .iter()
        .filter(|l| layer_has_math(&l.data))
        .all(|l| layer_equal(&l.data, lv1f1, lv1f2) && layer_equal(&l.data, lv2f1, lv2f2))
}

/// `contig_ldata_around_vert`: are all loop layers with math contiguous around `v`?
pub(crate) fn contig_ldata_around_vert(bm: &BMesh, v: VertId) -> bool {
    if bm.ldata.layers.is_empty() {
        return true;
    }
    let loops = bm.vert_loops(v);
    let Some((&l_first, rest)) = loops.split_first() else {
        return true;
    };
    rest.iter().all(|&l| contig_all_layers(bm, l_first, l))
}
