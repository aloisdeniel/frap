// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_face_split_edgenet_connect_islands` (`bmesh/intern/bmesh_polygon_edgenet.cc`, Blender
//! 5.2.4, with `USE_PARTIAL_CONNECT`): before `BM_face_split_edgenet`, connects the islands
//! of a wire net lying on a face (pieces touching neither the face boundary nor each other)
//! to the rest with new edges, so the net can split the face.
//!
//! The islands (the face boundary first) are sorted along the face's dominant 2D axis; each
//! island casts a ray towards −X from its lowest vertex and towards +X from its highest one
//! and connects to the closest unblocked vertex of an earlier island. Vertices where an
//! island only touches the rest at one point (partial connections) are split apart first
//! and spliced back afterwards.
//!
//! The edges are searched through the same k-DOP BVH as Blender's (`kdopbvh.rs`), so ties
//! (a ray through a shared vertex) resolve the same way.
//!
//! **Create/kill order**: new edges are appended (`BM_CREATE_NO_DOUBLE`); a partial
//! connection creates a temporary vertex (appended) that is spliced back (killed) at the
//! end, and a new edge that became a double through that splice is killed.

use rustc_hash::FxHashMap;

use super::bli::{self, V3};
use super::kdopbvh::BvhTree;
use crate::types::CreateFlag;
use crate::{BMesh, EdgeId, FaceId, VertId};

type V2 = [f32; 2];

fn sub2(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot2(a: V2, b: V2) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}
fn len_sq2(a: V2, b: V2) -> f32 {
    let d = sub2(a, b);
    dot2(d, d)
}

/// `line_point_factor_v2`.
pub fn line_point_factor_v2(p: V2, l1: V2, l2: V2) -> f32 {
    let u = sub2(l2, l1);
    let h = sub2(p, l1);
    let dot = dot2(u, u);
    if dot > 0.0 { dot2(u, h) / dot } else { 0.0 }
}

/// `normalize_v2` (returns the length).
fn normalize_v2(a: V2) -> (V2, f32) {
    let d = dot2(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let f = 1.0 / d;
        ([a[0] * f, a[1] * f], d)
    } else {
        ([0.0, 0.0], 0.0)
    }
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (isect_seg_seg_v2_point_ex).
/// `isect_seg_seg_v2_point_ex`: 1 with the point, -1 otherwise.
pub fn isect_seg_seg_v2_point_ex(
    mut v0: V2,
    mut v1: V2,
    mut v2: V2,
    mut v3: V2,
    endpoint_bias: f32,
) -> Option<V2> {
    let eps = 1e-6f32;
    let endpoint_min = -endpoint_bias;
    let endpoint_max = endpoint_bias + 1.0;
    let mut s10 = sub2(v1, v0);
    let s32 = sub2(v3, v2);
    let mut s30 = sub2(v3, v0);
    let d = bli::cross_v2v2(s10, s32);
    if d != 0.0 {
        let u = bli::cross_v2v2(s30, s32) / d;
        let v = bli::cross_v2v2(s10, s30) / d;
        if (u >= endpoint_min && u <= endpoint_max) && (v >= endpoint_min && v <= endpoint_max) {
            let vi_test = [v0[0] + s10[0] * u, v0[1] + s10[1] * u];
            let s_vi_v2 = sub2(vi_test, v2);
            let v = dot2(s32, s_vi_v2) / dot2(s32, s32);
            if v >= endpoint_min && v <= endpoint_max {
                return Some(vi_test);
            }
        }
        return None;
    }
    if bli::cross_v2v2(s10, s30) == 0.0 && bli::cross_v2v2(s32, s30) == 0.0 {
        if v0 == v1 {
            if len_sq2(v2, v3) > eps * eps {
                std::mem::swap(&mut v0, &mut v2);
                std::mem::swap(&mut v1, &mut v3);
                s10 = sub2(v1, v0);
                s30 = sub2(v3, v0);
            } else {
                if v0 == v2 {
                    return Some(v0);
                }
                return None;
            }
        }
        let s20 = sub2(v2, v0);
        let mut u_a = dot2(s20, s10) / dot2(s10, s10);
        let mut u_b = dot2(s30, s10) / dot2(s10, s10);
        if u_a > u_b {
            std::mem::swap(&mut u_a, &mut u_b);
        }
        if u_a > endpoint_max || u_b < endpoint_min {
            return None;
        }
        if 0.0f32.max(u_a) == 1.0f32.min(u_b) {
            let t = 0.0f32.max(u_a);
            return Some([v0[0] + s10[0] * t, v0[1] + s10[1] * t]);
        }
    }
    None
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (isect_ray_seg_v2).
/// `isect_ray_seg_v2`: the ray parameter of the hit.
pub fn isect_ray_seg_v2(ray_origin: V2, ray_direction: V2, v0: V2, v1: V2) -> Option<f32> {
    let v0_local = sub2(v0, ray_origin);
    let v1_local = sub2(v1, ray_origin);
    let s10 = sub2(v1_local, v0_local);
    let det = bli::cross_v2v2(ray_direction, s10);
    if det != 0.0 {
        let v = bli::cross_v2v2(v0_local, v1_local);
        let p = [(ray_direction[0] * v) / det, (ray_direction[1] * v) / det];
        let t = dot2(p, ray_direction) / dot2(ray_direction, ray_direction);
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(t >= 0.0) {
            return None;
        }
        let h = sub2(v1_local, p);
        let u = dot2(s10, h) / dot2(s10, s10);
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        return Some(t);
    }
    None
}

/// The per-call state of the connection search (2D coordinates by vertex index).
struct Ctx<'a> {
    bvh: &'a BvhTree,
    edge_arr: &'a [EdgeId],
    vidx: &'a FxHashMap<VertId, usize>,
    co: &'a [V2],
    valid: &'a mut [bool],
    vert_range: [usize; 2],
}

impl Ctx<'_> {
    fn ix(&self, v: VertId) -> usize {
        self.vidx[&v]
    }
    fn c(&self, v: VertId) -> V2 {
        self.co[self.ix(v)]
    }
    fn outside_range(&self, bm: &BMesh, e: EdgeId) -> bool {
        let i = self.ix(bm.edge(e).v[0]);
        i < self.vert_range[0] || i >= self.vert_range[1]
    }

    /// `edge_isect_verts_point_2d`.
    fn edge_isect_verts(&self, bm: &BMesh, e: EdgeId, v_a: VertId, v_b: VertId) -> Option<V2> {
        let [e1, e2] = bm.edge(e).v;
        let p = isect_seg_seg_v2_point_ex(self.c(v_a), self.c(v_b), self.c(e1), self.c(e2), 1e-4)?;
        (e1 != v_a && e2 != v_a && e1 != v_b && e2 != v_b).then_some(p)
    }

    /// `test_edges_isect_2d_vert`.
    fn isect_vert(
        &self,
        bm: &BMesh,
        new: &[EdgeId],
        v_origin: VertId,
        v_other: VertId,
    ) -> Option<EdgeId> {
        let (_, dist_orig) = normalize_v2(sub2(self.c(v_other), self.c(v_origin)));
        let (dir, _) = normalize_v2(sub2(self.c(v_other), self.c(v_origin)));
        let o = self.c(v_origin);
        // `bvhtree_test_edges_isect_2d_vert_cb`.
        let mut cb = |index: i32, hit_dist: &mut f32| -> bool {
            let e = self.edge_arr[index as usize];
            if let Some(co_isect) = self.edge_isect_verts(bm, e, v_origin, v_other) {
                let t = line_point_factor_v2(co_isect, self.c(v_origin), self.c(v_other));
                let dist_new = dist_orig * t;
                if dist_new < *hit_dist && dist_new > 0.0 && self.outside_range(bm, e) {
                    *hit_dist = dist_new;
                    return true;
                }
            }
            false
        };
        let hit = self
            .bvh
            .ray_cast([o[0], o[1], 0.0], [dir[0], dir[1], 0.0], dist_orig, &mut cb)
            .map(|i| i as usize);
        let mut e_hit = hit.map(|i| self.edge_arr[i]);
        if hit.is_none() {
            let mut t_best = 1.0f32;
            for &e in new {
                if let Some(co_isect) = self.edge_isect_verts(bm, e, v_origin, v_other) {
                    let t_test = line_point_factor_v2(co_isect, self.c(v_origin), self.c(v_other));
                    if t_test < t_best {
                        t_best = t_test;
                        e_hit = Some(e);
                    }
                }
            }
        }
        e_hit
    }

    /// `test_edges_isect_2d_ray`.
    fn isect_ray(&self, bm: &BMesh, new: &[EdgeId], v_origin: VertId, dir: V2) -> Option<EdgeId> {
        let o = self.c(v_origin);
        // `bvhtree_test_edges_isect_2d_ray_cb`.
        let cb = |index: i32, hit_dist: &mut f32| -> bool {
            let e = self.edge_arr[index as usize];
            let [e1, e2] = bm.edge(e).v;
            if let Some(dist_new) = isect_ray_seg_v2(o, dir, self.c(e1), self.c(e2))
                && dist_new < *hit_dist
                && dist_new > 0.0
                && e1 != v_origin
                && e2 != v_origin
                && self.outside_range(bm, e)
            {
                *hit_dist = dist_new;
                return true;
            }
            false
        };
        let mut hit_dist = f32::MAX / 2.0;
        let hit = self
            .bvh
            .ray_cast(
                [o[0], o[1], 0.0],
                [dir[0], dir[1], 0.0],
                hit_dist,
                &mut |i, d| {
                    let r = cb(i, d);
                    hit_dist = *d;
                    r
                },
            )
            .map(|i| i as usize);
        let mut e_hit = hit.map(|i| self.edge_arr[i]);
        if hit.is_some() {
            for &e in new {
                let [e1, e2] = bm.edge(e).v;
                if let Some(dist_new) = isect_ray_seg_v2(o, dir, self.c(e1), self.c(e2))
                    && e1 != v_origin
                    && e2 != v_origin
                    && dist_new < hit_dist
                {
                    hit_dist = dist_new;
                    e_hit = Some(e);
                }
            }
        }
        e_hit
    }

    /// `bm_face_split_edgenet_find_connection`: the index of the vertex to connect to.
    fn find_connection(
        &mut self,
        bm: &BMesh,
        new: &[EdgeId],
        v_origin: VertId,
        positive: bool,
    ) -> Option<usize> {
        let dir = [if positive { 1.0 } else { -1.0 }, 0.0];
        let mut e_hit = self.isect_ray(bm, new, v_origin, dir)?;
        let o = self.c(v_origin);
        let mut vert_search: Vec<VertId> = Vec::new();
        let mut blacklist: Vec<VertId> = Vec::new();
        let mut v_other: Option<VertId> = None;
        let mut v_other_fallback;
        loop {
            frap_math::budget::check();
            let [e1, e2] = bm.edge(e_hit).v;
            let pair = if len_sq2(o, self.c(e1)) > len_sq2(o, self.c(e2)) {
                [e1, e2]
            } else {
                [e2, e1]
            };
            for v_iter in pair {
                let i = self.ix(v_iter);
                if self.valid[i] {
                    let ok = if positive {
                        self.co[i][0] > o[0]
                    } else {
                        self.co[i][0] < o[0]
                    };
                    if ok {
                        vert_search.push(v_iter);
                        blacklist.push(v_iter);
                        self.valid[i] = false;
                    }
                }
            }
            v_other_fallback = v_other;
            v_other = vert_search.pop();
            match v_other {
                Some(vo) => match self.isect_vert(bm, new, v_origin, vo) {
                    Some(e) => e_hit = e,
                    None => break,
                },
                None => break,
            }
        }
        if v_other.is_none() {
            v_other = v_other_fallback;
        }
        for v in blacklist {
            let i = self.ix(v);
            self.valid[i] = true;
        }
        v_other.map(|v| self.ix(v))
    }
}

/// `test_tagged_and_notface` for the partial connection (tagged = in the net).
fn tagged_and_notface(
    bm: &BMesh,
    tagged: &rustc_hash::FxHashSet<EdgeId>,
    e: EdgeId,
    f: FaceId,
) -> bool {
    tagged.contains(&e) && !bm.edge_in_face(e, f)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon_edgenet.cc
// (bm_face_split_edgenet_partial_connect).
/// `bm_face_split_edgenet_partial_connect`: when `v_delimit` joins an island to the rest at a
/// single vertex, splits the island's edges onto a new vertex and returns it.
fn partial_connect(
    bm: &mut BMesh,
    edge_in: &mut rustc_hash::FxHashSet<EdgeId>,
    vert_in: &mut rustc_hash::FxHashSet<VertId>,
    v_delimit: VertId,
    f: FaceId,
) -> Option<VertId> {
    // `EDGE_NOT_IN_STACK` / `VERT_NOT_IN_STACK` are `edge_in` / `vert_in` membership.
    let mut e_delimit_list: Vec<EdgeId> = Vec::new();
    let mut e_face_init = None;
    for e in bm.vert_edges(v_delimit) {
        if edge_in.contains(&e) {
            e_delimit_list.push(e);
            if bm.edge(e).l.is_some() && bm.edge_in_face(e, f) {
                e_face_init = Some(e);
            }
        }
    }
    if e_delimit_list.len() <= 2 {
        return None;
    }
    let mut vert_stack: Vec<VertId> = vec![v_delimit];
    vert_in.remove(&v_delimit);
    {
        let first_e = e_face_init.unwrap_or_else(|| bm.vert(v_delimit).e.expect("net vertex"));
        let mut v_other = bm.edge_other_vert(first_e, v_delimit);
        let mut search = vec![v_other];
        if vert_in.remove(&v_other) {
            vert_stack.push(v_other);
        }
        while let Some(vo) = search.pop() {
            frap_math::budget::check();
            v_other = vo;
            for e_iter in bm.vert_edges(v_other) {
                let v_step = bm.edge_other_vert(e_iter, v_other);
                if edge_in.contains(&e_iter) && vert_in.remove(&v_step) {
                    search.push(v_step);
                    vert_stack.push(v_step);
                }
            }
        }
    }
    let mut is_delimit = false;
    for e_iter in bm.vert_edges(v_delimit) {
        let v_step = bm.edge_other_vert(e_iter, v_delimit);
        if vert_in.contains(&v_step) && !bm.edge_in_face(e_iter, f) {
            is_delimit = true;
        } else {
            edge_in.remove(&e_iter);
        }
    }
    let mut v_split = None;
    if is_delimit {
        let co = bm.vert(v_delimit).co;
        let vs = bm.vert_create(co, None, CreateFlag::empty());
        // `BM_vert_separate_tested_edges`.
        let tagged: rustc_hash::FxHashSet<EdgeId> = edge_in.clone();
        let mut moved: Vec<EdgeId> = Vec::new();
        for e in bm.vert_edges(v_delimit) {
            if tagged_and_notface(bm, &tagged, e, f) {
                moved.insert(0, e);
            }
        }
        for e in moved {
            bm.disk_vert_replace(e, vs, v_delimit);
        }
        vert_in.insert(vs);
        if bm.vert(vs).e.is_none() {
            bm.vert_kill(vs);
        } else {
            v_split = Some(vs);
        }
    }
    for v in vert_stack {
        vert_in.insert(v);
    }
    for e in e_delimit_list {
        edge_in.insert(e);
    }
    v_split
}

/// `BM_edge_find_double`.
pub fn edge_find_double(bm: &BMesh, e: EdgeId) -> Option<EdgeId> {
    let [v, v_other] = bm.edge(e).v;
    let mut e_iter = e;
    loop {
        frap_math::budget::check();
        e_iter = bm.disk_edge_next(e_iter, v);
        if e_iter == e {
            return None;
        }
        if bm.vert_in_edge(e_iter, v_other) {
            return Some(e_iter);
        }
    }
}

/// `BM_vert_splice_check_double_face`. Between the partial-connect split and this splice the
/// edges moved onto the temporary vertex keep loops whose `v` is still the original vertex
/// (`BM_vert_separate_tested_edges` only swaps the disk cycles, as in Blender), so this must
/// walk the disk and radial cycles the way Blender does, never `BM_LOOPS_OF_VERT` (whose
/// face-edge walk does not terminate on such a vertex).
fn splice_check_double_face(bm: &BMesh, v_a: VertId, v_b: VertId) -> bool {
    crate::ops::intersect::query::vert_splice_check_double_face(bm, v_a, v_b)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon_edgenet.cc
// (BM_face_split_edgenet_connect_islands).
/// `BM_face_split_edgenet_connect_islands(f, edge_net_init, use_partial_connect)`: the net
/// with its connecting edges appended (`r_edge_net_new`), or `None` when the net and the
/// face boundary form a single island (nothing to connect).
pub fn face_split_edgenet_connect_islands(
    bm: &mut BMesh,
    f: FaceId,
    edge_net_init: &[EdgeId],
    mut use_partial_connect: bool,
) -> Option<Vec<EdgeId>> {
    let edge_net_init_len = edge_net_init.len();
    let mut edge_arr: Vec<EdgeId> = edge_net_init.to_vec();
    for l in bm.face_loops(f) {
        edge_arr.push(bm.lp(l).e);
    }
    let edge_arr_len = edge_arr.len();
    let mut edge_in: rustc_hash::FxHashSet<EdgeId> = edge_arr.iter().copied().collect();
    let mut vert_in: rustc_hash::FxHashSet<VertId> = Default::default();
    for &e in &edge_arr {
        let [a, b] = bm.edge(e).v;
        vert_in.insert(a);
        vert_in.insert(b);
    }

    // Split out delimiting vertices: (v_orig, v_temp), most recent first.
    let mut temp_vert_pairs: Vec<(VertId, VertId)> = Vec::new();
    if use_partial_connect {
        for &e_init in &edge_arr[..edge_net_init_len] {
            for j in 0..2 {
                let v_delimit = bm.edge(e_init).v[j];
                while let Some(v_other) =
                    partial_connect(bm, &mut edge_in, &mut vert_in, v_delimit, f)
                {
                    temp_vert_pairs.insert(0, (v_delimit, v_other));
                }
            }
        }
        if temp_vert_pairs.is_empty() {
            use_partial_connect = false;
        }
    }

    // Groups (islands): scan `edge_arr` backwards so the face boundary comes first; the list
    // is built by prepending, so `groups` ends up last-found first.
    struct Group {
        edges: Vec<EdgeId>,
        vert_len: usize,
        min: VertId,
        max: VertId,
        min_axis: V2,
        max_axis: V2,
    }
    let mut groups: Vec<Group> = Vec::new();
    {
        let mut edge_index = edge_arr_len - 1;
        let mut edge_in_group_tot = 0usize;
        loop {
            let mut edge_links: Vec<EdgeId> = Vec::new();
            let mut unique_verts = 0usize;
            let mut vstack: Vec<VertId> = Vec::new();
            let v0 = bm.edge(edge_arr[edge_index]).v[0];
            vstack.push(v0);
            vert_in.remove(&v0);
            while let Some(v_iter) = vstack.pop() {
                frap_math::budget::check();
                unique_verts += 1;
                for e_iter in bm.vert_edges(v_iter) {
                    if edge_in.remove(&e_iter) {
                        edge_links.insert(0, e_iter);
                        let v_other = bm.edge_other_vert(e_iter, v_iter);
                        if vert_in.remove(&v_other) {
                            vstack.push(v_other);
                        }
                    }
                }
            }
            edge_in_group_tot += edge_links.len();
            let first = edge_links[0];
            let [a, b] = bm.edge(first).v;
            groups.insert(
                0,
                Group {
                    edges: edge_links,
                    vert_len: unique_verts,
                    min: a,
                    max: b,
                    min_axis: [0.0; 2],
                    max_axis: [0.0; 2],
                },
            );
            if edge_in_group_tot == edge_arr_len {
                break;
            }
            while !edge_in.contains(&edge_arr[edge_index]) {
                edge_index -= 1;
            }
        }
    }

    let finish = |bm: &mut BMesh,
                  temp_vert_pairs: &[(VertId, VertId)],
                  use_partial: bool,
                  mut net_new: Vec<EdgeId>,
                  is_new: &rustc_hash::FxHashSet<EdgeId>|
     -> Vec<EdgeId> {
        if use_partial {
            for &(v_orig, v_temp) in temp_vert_pairs {
                if bm.edge_exists(v_orig, v_temp).is_none()
                    && !splice_check_double_face(bm, v_orig, v_temp)
                {
                    bm.vert_splice(v_orig, v_temp);
                }
            }
            let mut len = net_new.len();
            let mut i = edge_net_init_len;
            while i < len {
                frap_math::budget::check();
                while edge_find_double(bm, net_new[i]).is_some() {
                    if is_new.contains(&net_new[i]) {
                        bm.edge_kill(net_new[i]);
                    }
                    len -= 1;
                    if i == len {
                        break;
                    }
                    net_new[i] = net_new[len];
                }
                i += 1;
            }
            net_new.truncate(len);
        }
        net_new
    };

    if groups.len() == 1 {
        let _ = finish(
            bm,
            &temp_vert_pairs,
            use_partial_connect,
            edge_net_init.to_vec(),
            &Default::default(),
        );
        return None;
    }

    let axis_mat: [V3; 3] = bli::axis_dominant_v3_to_m3(bm.face(f).no);
    let axis_pt_cmp = |a: V2, b: V2| -> i32 {
        if a[0] < b[0] {
            -1
        } else if a[0] > b[0] {
            1
        } else if a[1] < b[1] {
            -1
        } else if a[1] > b[1] {
            1
        } else {
            0
        }
    };
    // `group_arr`: filled in reverse list order so the boundary group is first.
    for g in &mut groups {
        let mut min_axis = [f32::MAX, f32::MAX];
        let mut max_axis = [-f32::MAX, -f32::MAX];
        for &e in &g.edges {
            for v_iter in bm.edge(e).v {
                let axis_value = bli::mul_v2_m3v3(&axis_mat, bm.vert(v_iter).co);
                if axis_pt_cmp(axis_value, min_axis) == -1 {
                    g.min = v_iter;
                    min_axis = axis_value;
                }
                if axis_pt_cmp(axis_value, max_axis) == 1 {
                    g.max = v_iter;
                    max_axis = axis_value;
                }
            }
        }
        g.min_axis = min_axis;
        g.max_axis = max_axis;
    }
    groups.reverse();
    let mut order: Vec<usize> = (0..groups.len()).collect();
    bli::c_qsort(&mut order, &|&a: &usize, &b: &usize| {
        let t = axis_pt_cmp(groups[a].min_axis, groups[b].min_axis);
        if t == 0 {
            axis_pt_cmp(groups[a].max_axis, groups[b].max_axis)
        } else {
            t
        }
    });
    let mut group_arr: Vec<Group> = Vec::with_capacity(groups.len());
    {
        let mut slots: Vec<Option<Group>> = groups.into_iter().map(Some).collect();
        for &i in &order {
            group_arr.push(slots[i].take().expect("each group once"));
        }
    }

    // Vertex array (group order), projected 2D coordinates relative to the first corner.
    let f_co_ref = bm.vert(bm.lp(bm.face(f).l_first).v).co;
    let mut vidx: FxHashMap<VertId, usize> = FxHashMap::default();
    let mut vert_arr: Vec<VertId> = Vec::new();
    let mut verts_group_table: Vec<usize> = Vec::new();
    let mut co2d: Vec<V2> = Vec::new();
    for (g_index, g) in group_arr.iter().enumerate() {
        for &e in &g.edges {
            for v_iter in bm.edge(e).v {
                if let std::collections::hash_map::Entry::Vacant(slot) = vidx.entry(v_iter) {
                    slot.insert(vert_arr.len());
                    let co = bm.vert(v_iter).co;
                    let rel = [
                        co[0] - f_co_ref[0],
                        co[1] - f_co_ref[1],
                        co[2] - f_co_ref[2],
                    ];
                    co2d.push(bli::mul_v2_m3v3(&axis_mat, rel));
                    vert_arr.push(v_iter);
                    verts_group_table.push(g_index);
                }
            }
        }
    }
    let mut valid = vec![true; vert_arr.len()];
    let mut bvh = BvhTree::new(1e-4, 8, 8);
    for (i, &e) in edge_arr.iter().enumerate() {
        let [a, b] = bm.edge(e).v;
        let (ca, cb) = (co2d[vidx[&a]], co2d[vidx[&b]]);
        bvh.insert(i as i32, &[[ca[0], ca[1], 0.0], [cb[0], cb[1], 0.0]]);
    }
    bvh.balance();

    // temp -> orig mapping.
    let mut remap: Vec<i64> = vec![-1; vert_arr.len()];
    if use_partial_connect {
        for &(v_orig, v_temp) in &temp_vert_pairs {
            remap[vidx[&v_temp]] = vidx[&v_orig] as i64;
        }
    }
    let overlap = |remap: &[i64], a: usize, b: usize| remap[a] == b as i64 || remap[b] == a as i64;

    let mut edge_net_new: Vec<EdgeId> = edge_net_init.to_vec();
    let mut is_new: rustc_hash::FxHashSet<EdgeId> = Default::default();
    let mut has_prev: Vec<bool> = vec![false; group_arr.len()];
    let mut vert_range = [0usize, group_arr[0].vert_len];
    for g_index in 1..group_arr.len() {
        frap_math::budget::check();
        vert_range[0] = vert_range[1];
        vert_range[1] += group_arr[g_index].vert_len;
        for (positive, v_origin) in [
            (false, group_arr[g_index].min),
            (true, group_arr[g_index].max),
        ] {
            if !positive && has_prev[g_index] {
                continue;
            }
            let index_other = {
                let mut cx = Ctx {
                    bvh: &bvh,
                    edge_arr: &edge_arr,
                    vidx: &vidx,
                    co: &co2d,
                    valid: &mut valid,
                    vert_range,
                };
                let new = edge_net_new[edge_net_init_len..].to_vec();
                cx.find_connection(bm, &new, v_origin, positive)
            };
            let Some(index_other) = index_other else {
                continue;
            };
            if !use_partial_connect || !overlap(&remap, vidx[&v_origin], index_other) {
                let v_end = vert_arr[index_other];
                let totedge_prev = bm.edges_num();
                if v_origin != v_end {
                    let e_new = bm.edge_create(v_origin, v_end, None, CreateFlag::NO_DOUBLE);
                    if bm.edges_num() != totedge_prev {
                        is_new.insert(e_new);
                    } else {
                        is_new.remove(&e_new);
                    }
                    edge_net_new.push(e_new);
                }
            }
            if positive {
                has_prev[verts_group_table[index_other]] = true;
            }
        }
    }
    Some(finish(
        bm,
        &temp_vert_pairs,
        use_partial_connect,
        edge_net_new,
        &is_new,
    ))
}
