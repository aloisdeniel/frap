// SPDX-License-Identifier: GPL-3.0-or-later
//! Construction and query helpers of `bmesh_construct.cc`, `bmesh_query.cc` and
//! `bmesh_polygon.cc` (Blender 5.2.4) that the WP 2.8 ops need and the BMesh core does not
//! provide: `BM_face_create_ngon_verts`, `BM_verts_sort_radial_plane`,
//! `BM_verts_calc_normal_from_cloud_ex`, `BM_face_copy_shared`, `BM_edges_from_verts`,
//! `BM_face_exists_overlap_subset`, `BM_edge_ordered_verts`, face area/centre helpers,
//! `BM_mesh_calc_face_groups` (edge stepping).

use rustc_hash::FxHashSet;

use super::bli::{self, V3};
use crate::types::CreateFlag;
use crate::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// `BM_edge_ordered_verts`: the vertices of `e` in the winding of its first face (`e->l`), or
/// `e`'s own order without faces.
pub fn edge_ordered_verts(bm: &BMesh, e: EdgeId) -> (VertId, VertId) {
    match bm.edge(e).l {
        Some(l) => {
            let ll = bm.lp(l);
            (ll.v, bm.lp(ll.next).v)
        }
        None => {
            let [a, b] = bm.edge(e).v;
            (a, b)
        }
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_construct.cc (BM_edges_from_verts).
/// `BM_edges_from_verts`: the edges of the closed vertex cycle `verts` (`edges[i]` joins
/// `verts[i]` and `verts[i + 1]`), `None` when one is missing.
pub fn edges_from_verts(bm: &BMesh, verts: &[VertId]) -> Option<Vec<EdgeId>> {
    let len = verts.len();
    let mut out = Vec::with_capacity(len);
    let mut i_prev = len - 1;
    for i in 0..len {
        out.push(bm.edge_exists(verts[i_prev], verts[i])?);
        i_prev = i;
    }
    // `edge_arr[i_prev]` ordering: Blender fills `edge_arr[i_prev] = edge(v[i_prev], v[i])`.
    out.rotate_left(1);
    Some(out)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_construct.cc
// (BM_face_create_ngon_verts).
/// `BM_face_create_ngon_verts`: a face over the ordered vertex cycle `verts`, winding chosen
/// against the existing faces of its edges when `calc_winding`, missing edges created when
/// `create_edges` (else `None` when one is missing).
pub fn face_create_ngon_verts(
    bm: &mut BMesh,
    verts: &[VertId],
    example: Option<FaceId>,
    flag: CreateFlag,
    calc_winding: bool,
    create_edges: bool,
) -> Option<FaceId> {
    let len = verts.len();
    debug_assert!(len > 2);
    let mut edge_arr = Vec::with_capacity(len);
    let mut winding = [0u32; 2];
    let mut i_prev = len - 1;
    let v_winding = [verts[i_prev], verts[0]];
    for i in 0..len {
        let e = if create_edges {
            bm.edge_create(verts[i_prev], verts[i], None, CreateFlag::NO_DOUBLE)
        } else {
            bm.edge_exists(verts[i_prev], verts[i])?
        };
        edge_arr.push(e);
        if calc_winding && bm.edge(e).l.is_some() {
            let (test_v2, _test_v1) = edge_ordered_verts(bm, e);
            winding[(verts[i_prev] == test_v2) as usize] += 1;
        }
        i_prev = i;
    }
    let (w0, w1) = if calc_winding && winding[0] < winding[1] {
        (1, 0)
    } else {
        (0, 1)
    };
    bm.face_create_ngon(v_winding[w0], v_winding[w1], &edge_arr, example, flag)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc
// (BM_verts_calc_normal_from_cloud_ex).
/// `BM_verts_calc_normal_from_cloud_ex`: (normal, centre, tangent vertex index).
pub fn verts_calc_normal_from_cloud_ex(bm: &BMesh, varr: &[VertId]) -> (V3, V3, usize) {
    let len = varr.len();
    let co = |i: usize| bm.vert(varr[i]).co;
    let len_inv = 1.0 / len as f32;
    let mut center = [0.0f32; 3];
    for i in 0..len {
        bli::madd_v3_v3fl(&mut center, co(i), len_inv);
    }
    let mut co_a_index = 0usize;
    let mut co_a: Option<usize> = None;
    {
        let mut dist_sq_max = -1.0f32;
        for i in 0..len {
            let d = bli::len_squared_v3v3(co(i), center);
            if !(d <= dist_sq_max) {
                co_a = Some(i);
                co_a_index = i;
                dist_sq_max = d;
            }
        }
    }
    let co_a = co_a.unwrap_or(0);
    let dir_a = bli::normalize_v3(bli::sub(co(co_a), center)).0;
    let mut co_b: Option<usize> = None;
    let mut dir_b = [0.0f32; 3];
    {
        let mut dist_sq_max = -1.0f32;
        for i in 0..len {
            // Blender compares the coordinate pointers (the same vertex).
            if i == co_a {
                continue;
            }
            let t = bli::project_plane_normalized_v3_v3v3(bli::sub(co(i), center), dir_a);
            let d = bli::len_squared_v3(t);
            if !(d <= dist_sq_max) {
                co_b = Some(i);
                dist_sq_max = d;
                dir_b = t;
            }
        }
    }
    let co_b_i = co_b.unwrap_or(co_a);
    let mut r_normal;
    if len <= 3 {
        r_normal = bli::normal_tri_v3(center, co(co_a), co(co_b_i)).0;
        return (r_normal, center, co_a_index);
    }
    dir_b = bli::normalize_v3(dir_b).0;
    let mut co_a_opp: Option<usize> = None;
    let mut co_b_opp: Option<usize> = None;
    {
        let mut dot_a_min = f32::MAX;
        let mut dot_b_min = f32::MAX;
        for i in 0..len {
            let c = co(i);
            if i != co_a {
                let d = bli::dot(dir_a, c);
                if d < dot_a_min || co_a_opp.is_none() {
                    dot_a_min = d;
                    co_a_opp = Some(i);
                }
            }
            if Some(i) != co_b {
                let d = bli::dot(dir_b, c);
                if d < dot_b_min || co_b_opp.is_none() {
                    dot_b_min = d;
                    co_b_opp = Some(i);
                }
            }
        }
    }
    let (n, l) = bli::normal_quad_v3(
        co(co_a),
        co(co_b_i),
        co(co_a_opp.unwrap_or(0)),
        co(co_b_opp.unwrap_or(0)),
    );
    r_normal = n;
    if l != 0.0 {
        let mut order: Vec<usize> = (0..len).collect();
        let angles: Vec<f32> = (0..len)
            .map(|i| {
                let a = bli::angle_signed_on_axis_v3v3_v3(dir_a, bli::sub(co(i), center), r_normal);
                if a.is_finite() { a } else { 0.0 }
            })
            .collect();
        order.sort_by(|&a, &b| angles[b].total_cmp(&angles[a]));
        let mut refine = [0.0f32; 3];
        let mut v_prev = co(order[len - 1]);
        for &o in &order {
            let v_curr = co(o);
            bli::add_newell_cross_v3_v3v3(&mut refine, v_prev, v_curr);
            v_prev = v_curr;
        }
        let (refine, rl) = bli::normalize_v3(refine);
        if rl != 0.0 {
            let mut dist_sq_max = -1.0f32;
            for i in 0..len {
                let t = bli::project_plane_normalized_v3_v3v3(bli::sub(co(i), center), refine);
                let d = bli::len_squared_v3(t);
                if !(d <= dist_sq_max) {
                    co_a_index = i;
                    dist_sq_max = d;
                }
            }
            r_normal = refine;
        }
    }
    (r_normal, center, co_a_index)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_construct.cc
// (BM_verts_sort_radial_plane).
/// `BM_verts_sort_radial_plane`: sorts `verts` by angle around their cloud normal.
pub fn verts_sort_radial_plane(bm: &BMesh, verts: &mut [VertId]) {
    let (nor, cent, index_tangent) = verts_calc_normal_from_cloud_ex(bm, verts);
    let far = bm.vert(verts[index_tangent]).co;
    let mut vang: Vec<(f32, usize)> = verts
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            (
                bli::angle_signed_on_axis_v3v3v3_v3(far, cent, bm.vert(v).co, nor),
                i,
            )
        })
        .collect();
    vang.sort_by(|a, b| a.0.total_cmp(&b.0));
    let orig = verts.to_vec();
    for (i, (_, k)) in vang.into_iter().enumerate() {
        verts[i] = orig[k];
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_construct.cc (BM_face_copy_shared).
/// `BM_face_copy_shared`: each corner of `f` takes the loop data of the matching corner of a
/// face sharing its edge (first match wins; `filter` tests the source loop).
pub fn face_copy_shared(bm: &mut BMesh, f: FaceId, filter: Option<&dyn Fn(&BMesh, LoopId) -> bool>) {
    let mut done: FxHashSet<LoopId> = FxHashSet::default();
    for l_iter in bm.face_loops(f) {
        let l_other = bm.lp(l_iter).radial_next;
        if l_other == l_iter {
            continue;
        }
        let l_dst = [l_iter, bm.lp(l_iter).next];
        let l_src = if bm.lp(l_other).v == bm.lp(l_iter).v {
            [l_other, bm.lp(l_other).next]
        } else {
            [bm.lp(l_other).next, l_other]
        };
        for j in 0..2 {
            if !done.contains(&l_dst[j]) && filter.is_none_or(|fl| fl(bm, l_src[j])) {
                bm.ldata.copy_elem(l_src[j].0, l_dst[j].0);
                done.insert(l_dst[j]);
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc
// (BM_face_exists_overlap_subset).
/// `BM_face_exists_overlap_subset`: some face uses only vertices of `varr` (with Blender's
/// exact search order, including its loop structure).
pub fn face_exists_overlap_subset(bm: &BMesh, varr: &[VertId]) -> bool {
    let len = varr.len();
    let vset: FxHashSet<VertId> = varr.iter().copied().collect();
    let mut seen: FxHashSet<FaceId> = FxHashSet::default();
    let mut is_overlap = false;
    for &v in varr {
        for f in bm.vert_faces(v) {
            if (bm.face(f).len as usize) <= len && !seen.contains(&f) {
                is_overlap = bm
                    .face_loops(f)
                    .into_iter()
                    .all(|l| vset.contains(&bm.lp(l).v));
                if is_overlap {
                    break;
                }
                seen.insert(f);
            }
        }
    }
    is_overlap
}

/// `BM_face_calc_area`: half the length of the Newell cross sum.
pub fn face_calc_area(bm: &BMesh, f: FaceId) -> f32 {
    let mut n = [0.0f32; 3];
    for l in bm.face_loops(f) {
        let a = bm.vert(bm.lp(l).v).co;
        let b = bm.vert(bm.lp(bm.lp(l).next).v).co;
        bli::add_newell_cross_v3_v3v3(&mut n, a, b);
    }
    bli::len_v3(n) * 0.5
}

/// `BM_face_calc_center_median_weighted`: corners weighted by the length of their two edges.
pub fn face_calc_center_median_weighted(bm: &BMesh, f: FaceId) -> V3 {
    let loops = bm.face_loops(f);
    let co = |l: LoopId| bm.vert(bm.lp(l).v).co;
    let l_first = loops[0];
    let mut w_prev = bli::len_v3v3(co(bm.lp(l_first).prev), co(l_first));
    let mut totw = 0.0f32;
    let mut r = [0.0f32; 3];
    for &l in &loops {
        let w_curr = bli::len_v3v3(co(l), co(bm.lp(l).next));
        let w = w_curr + w_prev;
        bli::madd_v3_v3fl(&mut r, co(l), w);
        totw += w;
        w_prev = w_curr;
    }
    if totw != 0.0 {
        r = bli::scale(r, 1.0 / totw);
    }
    r
}

/// `BM_face_calc_center_bounds`.
pub fn face_calc_center_bounds(bm: &BMesh, f: FaceId) -> V3 {
    let mut min = [f32::MAX; 3];
    let mut max = [-f32::MAX; 3];
    for l in bm.face_loops(f) {
        let c = bm.vert(bm.lp(l).v).co;
        for k in 0..3 {
            min[k] = min[k].min(c[k]);
            max[k] = max[k].max(c[k]);
        }
    }
    bli::mid_v3_v3v3(min, max)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_mesh_calc_face_groups,
// edge stepping). The groups' faces in Blender's visiting order; `filter` tests the stepping
// loop. Like Blender, every face ends with `BM_ELEM_TAG` set.
/// `BM_mesh_calc_face_groups(..., htype_step = BM_EDGE)`.
pub fn mesh_calc_face_groups(
    bm: &mut BMesh,
    filter: &dyn Fn(&BMesh, LoopId) -> bool,
) -> Vec<Vec<FaceId>> {
    let faces = bm.face_ids();
    for &f in &faces {
        bm.face_mut(f).flag.remove(ElemFlag::TAG);
    }
    let mut groups = Vec::new();
    let mut next = 0usize;
    let mut touched = 0usize;
    while touched != faces.len() {
        let mut stack = Vec::new();
        while next < faces.len() {
            let f = faces[next];
            next += 1;
            if !bm.face(f).flag.contains(ElemFlag::TAG) {
                bm.face_mut(f).flag.insert(ElemFlag::TAG);
                stack.push(f);
                break;
            }
        }
        let mut group = Vec::new();
        while let Some(f) = stack.pop() {
            group.push(f);
            touched += 1;
            for l_iter in bm.face_loops(f) {
                let first_r = bm.lp(l_iter).radial_next;
                if first_r != l_iter && filter(bm, l_iter) {
                    let mut l_r = first_r;
                    loop {
                        let fo = bm.lp(l_r).f;
                        if !bm.face(fo).flag.contains(ElemFlag::TAG) {
                            bm.face_mut(fo).flag.insert(ElemFlag::TAG);
                            stack.push(fo);
                        }
                        l_r = bm.lp(l_r).radial_next;
                        if l_r == l_iter {
                            break;
                        }
                    }
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// `BM_edge_share_vert_check`.
pub fn edge_share_vert_check(bm: &BMesh, e1: EdgeId, e2: EdgeId) -> bool {
    let a = bm.edge(e1).v;
    let b = bm.edge(e2).v;
    a[0] == b[0] || a[0] == b[1] || a[1] == b[0] || a[1] == b[1]
}

/// `BM_edge_share_face_check`.
pub fn edge_share_face_check(bm: &BMesh, e1: EdgeId, e2: EdgeId) -> bool {
    bm.edge_loops(e1)
        .into_iter()
        .any(|l| bm.edge_in_face(e2, bm.lp(l).f))
}

/// `BM_vert_in_face`.
pub fn vert_in_face(bm: &BMesh, v: VertId, f: FaceId) -> bool {
    bm.face_loops(f).into_iter().any(|l| bm.lp(l).v == v)
}

/// `BM_vert_is_wire`: a vertex with edges, none of them with a face.
pub fn vert_is_wire(bm: &BMesh, v: VertId) -> bool {
    match bm.vert(v).e {
        None => false,
        Some(_) => bm.vert_edges(v).into_iter().all(|e| bm.edge(e).l.is_none()),
    }
}

/// `BM_mesh_elem_hflag_disable_all` for the given flag on edges.
pub fn edges_flag_disable_all(bm: &mut BMesh, flag: ElemFlag) {
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(flag);
    }
}

/// `BM_mesh_elem_hflag_disable_all` for the given flag on faces.
pub fn faces_flag_disable_all(bm: &mut BMesh, flag: ElemFlag) {
    for f in bm.faces.slots.iter_mut().flatten() {
        f.flag.remove(flag);
    }
}

/// `BM_mesh_elem_hflag_disable_all` for the given flag on vertices.
pub fn verts_flag_disable_all(bm: &mut BMesh, flag: ElemFlag) {
    for v in bm.verts.slots.iter_mut().flatten() {
        v.flag.remove(flag);
    }
}
