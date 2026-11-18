// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_polyfill_beautify`: improves a polyfill triangulation of a simple 2D polygon by
//! rotating internal edges (lowest cost first) until no rotation helps, plus the quad
//! rotation costs `BM_face_triangulate` and `join_triangles` share.
//!
//! Ported from blender/source/blender/blenlib/intern/polyfill_2d_beautify.cc (Blender 5.2.4).

use super::heap::{Heap, NodeId};
use super::math::{cross_tri_v2, cross_tri_v3, len_v2v2, normalize_v3};
use crate::query::geom::{V3, add};

/// `BLI_polyfill_beautify_quad_rotate_calc_ex`: negative when splitting the quad `v1..v4`
/// along 1–3 beats the current 2–4 split (`FLT_MAX`: never rotate, `-FLT_MAX`: always).
/// Also returns the area estimate used to scale the epsilon.
pub(crate) fn quad_rotate_calc_ex(
    v1: [f32; 2],
    v2: [f32; 2],
    v3: [f32; 2],
    v4: [f32; 2],
    lock_degenerate: bool,
) -> (f32, f32) {
    let eps_zero_area = 1e-12f32;
    let area_2x_234 = cross_tri_v2(v2, v3, v4);
    let area_2x_241 = cross_tri_v2(v2, v4, v1);
    let area_2x_123 = cross_tri_v2(v1, v2, v3);
    let area_2x_134 = cross_tri_v2(v1, v3, v4);
    let area =
        (area_2x_234.abs() + area_2x_241.abs() + area_2x_123.abs() + area_2x_134.abs()) / 8.0;

    // Test for unusable (1-3) state.
    if (area_2x_123 >= 0.0) != (area_2x_134 >= 0.0) {
        return (f32::MAX, area);
    }
    if area_2x_123.abs() <= eps_zero_area || area_2x_134.abs() <= eps_zero_area {
        return (f32::MAX, area);
    }
    // Test for unusable (2-4) state (same as above).
    if (area_2x_234 >= 0.0) != (area_2x_241 >= 0.0) {
        if lock_degenerate {
            return (f32::MAX, area);
        }
        return (-f32::MAX, area);
    }
    if area_2x_234.abs() <= eps_zero_area || area_2x_241.abs() <= eps_zero_area {
        return (-f32::MAX, area);
    }

    let len_12 = len_v2v2(v1, v2);
    let len_23 = len_v2v2(v2, v3);
    let len_34 = len_v2v2(v3, v4);
    let len_41 = len_v2v2(v4, v1);
    let len_13 = len_v2v2(v1, v3);
    let len_24 = len_v2v2(v2, v4);

    // Edge (2-4), current state.
    let area_a = area_2x_234.abs();
    let area_b = area_2x_241.abs();
    let prim_a = len_23 + len_34 + len_24;
    let prim_b = len_41 + len_12 + len_24;
    let fac_24 = (area_a / prim_a) + (area_b / prim_b);

    // Edge (1-3), new state.
    let area_a = area_2x_123.abs();
    let area_b = area_2x_134.abs();
    let prim_a = len_12 + len_23 + len_13;
    let prim_b = len_34 + len_41 + len_13;
    let fac_13 = (area_a / prim_a) + (area_b / prim_b);

    (fac_24 - fac_13, area)
}

/// `signum_i_ex(a, eps)`.
fn signum_i_ex(a: f32, eps: f32) -> i32 {
    if a > eps {
        1
    } else if a < -eps {
        -1
    } else {
        0
    }
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (axis_dominant_v3_to_m3).
/// `axis_dominant_v3_to_m3` followed by `mul_v2_m3v3`: the 2D projection of `p` on the plane
/// of the unit normal `n`.
pub(crate) fn axis_dominant_project(n: V3) -> impl Fn(V3) -> [f32; 2] {
    let (x, y) = frap_geom::polyfill::ortho_basis_v3v3_v3(n);
    // Rows of the transposed matrix (columns x, y, n).
    move |p: V3| {
        [
            x[0] * p[0] + x[1] * p[1] + x[2] * p[2],
            y[0] * p[0] + y[1] * p[1] + y[2] * p[2],
        ]
    }
}

/// `BLI_polyfill_edge_calc_rotate_beauty__area`: the cost of rotating the edge 2–4 of the
/// quad `v1..v4` (3D) to 1–3, measured in the plane of the quad's average normal.
pub(crate) fn edge_calc_rotate_beauty_area(
    v1: V3,
    v2: V3,
    v3: V3,
    v4: V3,
    lock_degenerate: bool,
) -> f32 {
    let eps = 1e-5f32;
    let no_a = cross_tri_v3(v2, v3, v4);
    let no_b = cross_tri_v3(v2, v4, v1);
    let (no, no_scale) = normalize_v3(add(no_a, no_b));
    if no_scale == 0.0 {
        return f32::MAX;
    }
    let proj = axis_dominant_project(no);
    let (v1_xy, v2_xy, v3_xy, v4_xy) = (proj(v1), proj(v2), proj(v3), proj(v4));
    if signum_i_ex(cross_tri_v2(v2_xy, v3_xy, v4_xy) / no_scale, eps)
        + signum_i_ex(cross_tri_v2(v2_xy, v4_xy, v1_xy) / no_scale, eps)
        == 0
    {
        return f32::MAX;
    }
    quad_rotate_calc_ex(v1_xy, v2_xy, v3_xy, v4_xy, lock_degenerate).0
}

#[derive(Copy, Clone, Debug)]
struct HalfEdge {
    v: u32,
    e_next: u32,
    e_radial: u32,
    base_index: u32,
}

#[derive(Copy, Clone, Debug)]
struct OrderEdge {
    verts: [u32; 2],
    e_half: u32,
}

fn is_boundary_edge(i_a: u32, i_b: u32, coord_last: u32) -> bool {
    i_a + 1 == i_b || (i_a == 0 && i_b == coord_last)
}

fn rotate_beauty_calc(coords: &[[f32; 2]], edges: &[HalfEdge], e_a: usize) -> (f32, f32) {
    let e_b = edges[e_a].e_radial as usize;
    let e_a_other = edges[edges[edges[e_a].e_next as usize].e_next as usize];
    let e_b_other = edges[edges[edges[e_b].e_next as usize].e_next as usize];
    let v1 = coords[e_a_other.v as usize];
    let v2 = coords[edges[e_a].v as usize];
    let v3 = coords[e_b_other.v as usize];
    let v4 = coords[edges[e_b].v as usize];
    quad_rotate_calc_ex(v1, v2, v3, v4, false)
}

fn cost_update_single(
    coords: &[[f32; 2]],
    edges: &[HalfEdge],
    e: usize,
    heap: &mut Heap<usize>,
    table: &mut [Option<NodeId>],
) {
    let i = edges[e].base_index as usize;
    let (cost, area) = rotate_beauty_calc(coords, edges, e);
    if cost < -1e-6f32 * area.max(1.0) {
        heap.insert_or_update(&mut table[i], cost, e);
    } else if let Some(n) = table[i].take() {
        heap.remove(n);
    }
}

fn cost_update(
    coords: &[[f32; 2]],
    edges: &[HalfEdge],
    e: usize,
    heap: &mut Heap<usize>,
    table: &mut [Option<NodeId>],
) {
    let e0 = edges[e].e_next as usize;
    let e1 = edges[e0].e_next as usize;
    let er = edges[e].e_radial as usize;
    let e2 = edges[er].e_next as usize;
    let e3 = edges[e2].e_next as usize;
    for x in [e0, e1, e2, e3] {
        if edges[x].base_index != u32::MAX {
            cost_update_single(coords, edges, x, heap, table);
        }
    }
}

fn rotate(edges: &mut [HalfEdge], e: usize) {
    let mut ed = [0usize; 6];
    ed[0] = e;
    ed[1] = edges[ed[0]].e_next as usize;
    ed[2] = edges[ed[1]].e_next as usize;
    ed[3] = edges[e].e_radial as usize;
    ed[4] = edges[ed[3]].e_next as usize;
    ed[5] = edges[ed[4]].e_next as usize;

    edges[ed[0]].e_next = ed[2] as u32;
    edges[ed[1]].e_next = ed[3] as u32;
    edges[ed[2]].e_next = ed[4] as u32;
    edges[ed[3]].e_next = ed[5] as u32;
    edges[ed[4]].e_next = ed[0] as u32;
    edges[ed[5]].e_next = ed[1] as u32;

    edges[ed[0]].v = edges[ed[5]].v;
    edges[ed[3]].v = edges[ed[2]].v;
}

/// `BLI_polyfill_beautify(coords, tris)`: rewrites `tris` (a polyfill result over `coords`).
pub(crate) fn polyfill_beautify(coords: &[[f32; 2]], tris: &mut [[u32; 3]]) {
    let coords_num = coords.len() as u32;
    if coords_num < 4 {
        return;
    }
    let coord_last = coords_num - 1;
    let tris_len = (coords_num - 2) as usize;
    let edges_len = tris_len - 1;
    let half_edges_len = 3 * tris_len;
    let mut half_edges = vec![
        HalfEdge {
            v: 0,
            e_next: 0,
            e_radial: u32::MAX,
            base_index: u32::MAX,
        };
        half_edges_len
    ];
    let mut order_edges: Vec<OrderEdge> = Vec::with_capacity(2 * edges_len);

    for (i, tri) in tris.iter().enumerate().take(tris_len) {
        let mut j_prev = 2usize;
        for j_curr in 0..3usize {
            let e_index_prev = (i * 3 + j_prev) as u32;
            let e_index_curr = (i * 3 + j_curr) as u32;
            let he = &mut half_edges[e_index_prev as usize];
            he.v = tri[j_prev];
            he.e_next = e_index_curr;
            he.e_radial = u32::MAX;
            he.base_index = u32::MAX;
            let mut e_pair = [tri[j_prev], tri[j_curr]];
            if e_pair[0] > e_pair[1] {
                e_pair.swap(0, 1);
            }
            if !is_boundary_edge(e_pair[0], e_pair[1], coord_last) {
                order_edges.push(OrderEdge {
                    verts: e_pair,
                    e_half: e_index_prev,
                });
            }
            j_prev = j_curr;
        }
    }
    // `qsort` with `oedge_cmp`: no two entries compare equal, so any sort gives this order.
    order_edges.sort_by(|a, b| {
        a.verts[0]
            .cmp(&b.verts[0])
            .then(a.verts[1].cmp(&b.verts[1]))
            .then(a.e_half.cmp(&b.e_half))
    });
    let mut base_index = 0u32;
    for pair in order_edges.chunks_exact(2) {
        let (a, b) = (pair[0], pair[1]);
        half_edges[a.e_half as usize].e_radial = b.e_half;
        half_edges[b.e_half as usize].e_radial = a.e_half;
        half_edges[a.e_half as usize].base_index = base_index;
        half_edges[b.e_half as usize].base_index = base_index;
        base_index += 1;
    }

    let mut heap: Heap<usize> = Heap::new();
    let mut table: Vec<Option<NodeId>> = vec![None; edges_len.max(1)];
    for i in 0..half_edges_len {
        let e = half_edges[i];
        // Accounts for boundary edges too (UINT_MAX).
        if (e.e_radial as usize) < i {
            let (cost, _) = rotate_beauty_calc(coords, &half_edges, i);
            table[e.base_index as usize] = if cost < 0.0 {
                Some(heap.insert(cost, i))
            } else {
                None
            };
        }
    }
    while !heap.is_empty() {
        frap_math::budget::check();
        let e = heap.pop_min();
        table[half_edges[e].base_index as usize] = None;
        rotate(&mut half_edges, e);
        cost_update(coords, &half_edges, e, &mut heap, &mut table);
    }

    let mut tri_index = 0usize;
    for i in 0..half_edges_len {
        if half_edges[i].v != u32::MAX {
            let mut e = i;
            let tri = &mut tris[tri_index];
            tri_index += 1;
            for t in tri.iter_mut() {
                *t = half_edges[e].v;
                half_edges[e].v = u32::MAX;
                e = half_edges[e].e_next as usize;
            }
        }
    }
}
