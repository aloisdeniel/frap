// SPDX-License-Identifier: GPL-3.0-or-later
//! Edge-loop utilities (`bmesh/intern/bmesh_edgeloop.cc`, Blender 5.2.4): `BMEdgeLoopStore`,
//! `BM_mesh_edgeloops_find`, `BM_mesh_edgeloops_find_path`, centres, normals, ordering,
//! flipping, expansion and overlap tests. Used by `bridge_loops`, `grid_fill` and the
//! `mesh.fill_grid` operator.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ops::contextual_create::bli::{self, V3};
use crate::{BMesh, EdgeId, VertId};

/// An edge predicate (`bool (*test_fn)(BMEdge *, void *user_data)`).
pub type EdgeTest<'a> = &'a dyn Fn(&BMesh, EdgeId) -> bool;

/// Normals of very small loops are still wanted.
const EDGELOOP_EPS: f32 = 1e-10;

/// `BMEdgeLoopStore`: an ordered chain of vertices, optionally closed.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeLoopStore {
    /// The vertices in loop order (`verts`, a `ListBase` of `LinkData`).
    pub verts: Vec<VertId>,
    /// `BM_EDGELOOP_IS_CLOSED`.
    pub closed: bool,
    /// Centre (`BM_edgeloop_calc_center`).
    pub co: V3,
    /// Normal (`BM_edgeloop_calc_normal*`).
    pub no: V3,
}

impl EdgeLoopStore {
    /// `BM_edgeloop_from_verts`.
    pub fn from_verts(verts: Vec<VertId>, closed: bool) -> Self {
        EdgeLoopStore {
            verts,
            closed,
            co: [0.0; 3],
            no: [0.0; 3],
        }
    }
    /// `BM_edgeloop_length_get`.
    pub fn len(&self) -> usize {
        self.verts.len()
    }
    /// Whether the loop has no vertex.
    pub fn is_empty(&self) -> bool {
        self.verts.is_empty()
    }
    /// `BM_EDGELINK_NEXT`: the index after `i` (wrapping when closed).
    pub fn next_index(&self, i: usize) -> Option<usize> {
        if i + 1 < self.verts.len() {
            Some(i + 1)
        } else if self.closed {
            Some(0)
        } else {
            None
        }
    }
    /// `BM_edgeloop_edges_get`.
    pub fn edges(&self, bm: &BMesh) -> Vec<EdgeId> {
        let mut out = Vec::with_capacity(self.verts.len());
        for w in self.verts.windows(2) {
            out.push(bm.edge_exists(w[0], w[1]).expect("edge loop edge"));
        }
        if self.closed {
            out.push(
                bm.edge_exists(self.verts[0], *self.verts.last().expect("non-empty"))
                    .expect("edge loop closing edge"),
            );
        }
        out
    }
    /// `BM_edgeloop_calc_center`: edge-length weighted centre.
    pub fn calc_center(&mut self, bm: &BMesh) {
        let n = self.verts.len();
        let co = |i: usize| bm.vert(self.verts[i]).co;
        // node_prev = last->prev, node_curr = last, node_next = first.
        let mut v_prev = co(n - 2);
        let mut v_curr = co(n - 1);
        let mut v_next = co(0);
        let mut next_i = 0usize;
        let mut totw = 0.0f32;
        let mut c = [0.0f32; 3];
        let mut w_prev = bli::len_v3v3(v_prev, v_curr);
        loop {
            let w_curr = bli::len_v3v3(v_curr, v_next);
            let w = w_curr + w_prev;
            bli::madd_v3_v3fl(&mut c, v_curr, w);
            totw += w;
            w_prev = w_curr;
            next_i += 1;
            if next_i >= n {
                break;
            }
            v_prev = v_curr;
            v_curr = v_next;
            v_next = co(next_i);
        }
        let _ = v_prev;
        if totw != 0.0 {
            c = bli::scale(c, 1.0 / totw);
        }
        self.co = c;
    }
    /// `BM_edgeloop_calc_normal` (Newell); false (normal +Z) when degenerate.
    pub fn calc_normal(&mut self, bm: &BMesh) -> bool {
        let mut no = [0.0f32; 3];
        let mut v_prev = bm.vert(*self.verts.last().expect("non-empty")).co;
        for &v in &self.verts {
            let v_curr = bm.vert(v).co;
            bli::add_newell_cross_v3_v3v3(&mut no, v_prev, v_curr);
            v_prev = v_curr;
        }
        let (n, l) = bli::normalize_v3(no);
        if l < EDGELOOP_EPS {
            self.no = [0.0, 0.0, 1.0];
            return false;
        }
        self.no = n;
        true
    }
    /// `BM_edgeloop_calc_normal_aligned`.
    pub fn calc_normal_aligned(&mut self, bm: &BMesh, no_align: V3) -> bool {
        let mut no = [0.0f32; 3];
        let mut v_prev = bm.vert(*self.verts.last().expect("non-empty")).co;
        for &v in &self.verts {
            let v_curr = bm.vert(v).co;
            let dir = bli::sub(v_curr, v_prev);
            let cr = bli::cross(no_align, dir);
            let n = bli::cross(dir, cr);
            no = bli::add(no, n);
            v_prev = v_curr;
        }
        let (n, l) = bli::normalize_v3(no);
        if l < EDGELOOP_EPS {
            self.no = [0.0, 0.0, 1.0];
            return false;
        }
        self.no = n;
        true
    }
    /// `BM_edgeloop_flip`.
    pub fn flip(&mut self) {
        self.no = bli::negate(self.no);
        self.verts.reverse();
    }
    /// `BLI_listbase_rotate_first(verts, verts[i])`.
    pub fn rotate_first(&mut self, i: usize) {
        self.verts.rotate_left(i);
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_edgeloop.cc (BM_edgeloop_expand).
    /// `BM_edgeloop_expand(el_store_len, split = false)`: duplicates entries evenly until the
    /// loop has `len` entries.
    pub fn expand(&mut self, len: usize) {
        // First double until we are more than half as big.
        while self.verts.len() * 2 < len {
            let mut out = Vec::with_capacity(self.verts.len() * 2);
            for &v in &self.verts {
                out.push(v);
                out.push(v);
            }
            self.verts = out;
        }
        if self.verts.len() < len {
            let src = self.verts.len() as i32;
            let dst = (len - self.verts.len()) as i32;
            let picks: FxHashSet<i32> = bli::sparse_range(src, dst).into_iter().collect();
            let mut out = Vec::with_capacity(len);
            for (i, &v) in self.verts.iter().enumerate() {
                out.push(v);
                if picks.contains(&(i as i32)) {
                    out.push(v);
                }
            }
            self.verts = out;
        }
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_edgeloop.cc (BM_edgeloop_expand,
    // EDGE_SPLIT).
    /// `BM_edgeloop_expand(el_store_len, split = true)`: grows the loop to `len` entries by
    /// splitting its edges (`BM_edge_split` at factor 0, new vertices at the start of the
    /// split edge); the new edges are appended to `split_edges` (to be collapsed later).
    pub fn expand_split(&mut self, bm: &mut BMesh, len: usize, split_edges: &mut Vec<EdgeId>) {
        let mut split_swap = true;
        let mut edge_split = |bm: &mut BMesh, v_copy: VertId, v_other: VertId, swap: bool| {
            let e_other = bm.edge_exists(v_copy, v_other).expect("edge loop edge");
            let (v_split, e_split) =
                bm.edge_split(e_other, if swap { v_copy } else { v_other }, 0.0);
            bm.vert_mut(v_split).e = Some(e_split);
            if !split_edges.contains(&e_split) {
                split_edges.push(e_split);
            }
            v_split
        };
        while self.verts.len() * 2 < len {
            let mut i = 0usize;
            while i < self.verts.len() {
                let v = self.verts[i];
                let has_next = i + 1 < self.verts.len();
                if has_next || self.closed {
                    let other = if has_next { self.verts[i + 1] } else { self.verts[0] };
                    let vs = edge_split(bm, v, other, split_swap);
                    self.verts.insert(i + 1, vs);
                    i += 2;
                } else {
                    let other = self.verts[i - 1];
                    let vs = edge_split(bm, v, other, split_swap);
                    self.verts.insert(i, vs);
                    i += 2;
                }
                split_swap = !split_swap;
            }
            split_swap = !split_swap;
        }
        if self.verts.len() < len {
            let src = self.verts.len() as i32;
            let dst = (len - self.verts.len()) as i32;
            let picks = bli::sparse_range(src, dst);
            // `picks` are original indices; `shift` counts the entries inserted before them.
            for (shift, p) in picks.into_iter().enumerate() {
                let i = p as usize + shift;
                let v = self.verts[i];
                let has_next = i + 1 < self.verts.len();
                if has_next || self.closed {
                    let other = if has_next { self.verts[i + 1] } else { self.verts[0] };
                    let vs = edge_split(bm, v, other, split_swap);
                    self.verts.insert(i + 1, vs);
                } else {
                    let other = self.verts[i - 1];
                    let vs = edge_split(bm, v, other, split_swap);
                    self.verts.insert(i, vs);
                }
                split_swap = !split_swap;
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_edgeloop.cc (bm_vert_other_tag,
// bm_loop_build, BM_mesh_edgeloops_find).
/// `BM_mesh_edgeloops_find`: the chains/loops of edges passing `test` (vertices with more
/// than two such edges break a loop and discard it).
pub fn mesh_edgeloops_find(bm: &BMesh, test: &dyn Fn(&BMesh, EdgeId) -> bool) -> Vec<EdgeLoopStore> {
    let mut tag_e: FxHashSet<EdgeId> = FxHashSet::default();
    let mut tag_v: FxHashSet<VertId> = FxHashSet::default();
    let mut edges = Vec::new();
    for e in bm.edge_ids() {
        if test(bm, e) {
            tag_e.insert(e);
            let [a, b] = bm.edge(e).v;
            tag_v.insert(a);
            tag_v.insert(b);
            edges.push(e);
        }
    }
    let other_tag = |tag_e: &FxHashSet<EdgeId>, v: VertId, v_prev: VertId| -> (u32, Option<EdgeId>) {
        let mut count = 0;
        let mut e_next = None;
        for e in bm.vert_edges(v) {
            if tag_e.contains(&e) && bm.edge_other_vert(e, v) != v_prev {
                e_next = Some(e);
                count += 1;
            }
        }
        (count, e_next)
    };
    // `bm_loop_build`: returns false on a branch.
    let build = |tag_e: &mut FxHashSet<EdgeId>,
                 tag_v: &mut FxHashSet<VertId>,
                 el: &mut EdgeLoopStore,
                 mut v_prev: VertId,
                 v: VertId,
                 dir: i32|
     -> bool {
        if !tag_v.contains(&v) {
            return true;
        }
        let v_first = v;
        let mut cur = Some(v);
        while let Some(v) = cur {
            if dir == 1 {
                el.verts.insert(0, v);
            } else {
                el.verts.push(v);
            }
            tag_v.remove(&v);
            let (count, e_next) = other_tag(tag_e, v, v_prev);
            let v_next = if count == 1 {
                let en = e_next.expect("count 1");
                let vn = bm.edge_other_vert(en, v);
                tag_e.remove(&en);
                if vn == v_first {
                    el.closed = true;
                    None
                } else {
                    Some(vn)
                }
            } else if count == 0 {
                None
            } else {
                return false;
            };
            v_prev = v;
            cur = v_next;
        }
        true
    };
    let mut out = Vec::new();
    for &e in &edges {
        if tag_e.contains(&e) {
            let mut el = EdgeLoopStore::from_verts(Vec::new(), false);
            let [v1, v2] = bm.edge(e).v;
            if build(&mut tag_e, &mut tag_v, &mut el, v1, v2, 1)
                && build(&mut tag_e, &mut tag_v, &mut el, v2, v1, -1)
                && el.verts.len() > 1
            {
                out.push(el);
            }
        }
    }
    out
}

// Ported from blender/source/blender/bmesh/intern/bmesh_edgeloop.cc (vs_add,
// bm_loop_path_build_step, BM_mesh_edgeloops_find_path).
/// `BM_mesh_edgeloops_find_path`: the shortest open chain from `v_src` to `v_dst` over edges
/// passing `test` (all edges when `None`). Like Blender, it re-points `v->e` of the visited
/// vertices to the edge they were reached by.
pub fn mesh_edgeloops_find_path(
    bm: &mut BMesh,
    test: Option<EdgeTest<'_>>,
    v_src: VertId,
    v_dst: VertId,
) -> Option<EdgeLoopStore> {
    let mut index: FxHashMap<VertId, i32> = FxHashMap::default();
    let mut tag_e: FxHashSet<EdgeId> = FxHashSet::default();
    for e in bm.edge_ids() {
        if test.is_none_or(|t| t(bm, e)) {
            tag_e.insert(e);
        }
    }
    let idx = |index: &FxHashMap<VertId, i32>, v: VertId| index.get(&v).copied().unwrap_or(0);
    let mut lb_src: Vec<VertId> = Vec::new();
    let mut lb_dst: Vec<VertId> = Vec::new();
    // `vs_add(v, e_prev, iter_tot)` with dummy edges for the two seeds.
    index.insert(v_src, 1);
    lb_src.push(v_src);
    index.insert(v_dst, -1);
    lb_dst.push(v_dst);
    let mut v_match: Option<(VertId, VertId)> = None;

    let step = |bm: &mut BMesh,
                    index: &mut FxHashMap<VertId, i32>,
                    tag_e: &mut FxHashSet<EdgeId>,
                    lb: &mut Vec<VertId>,
                    dir: i32|
     -> (bool, Option<(VertId, VertId)>) {
        let mut lb_tmp = Vec::new();
        for &vs in lb.iter() {
            let vs_iter_next = idx(index, vs) + dir;
            for e in bm.vert_edges(vs) {
                if tag_e.contains(&e) {
                    let v_next = bm.edge_other_vert(e, vs);
                    let v_next_index = idx(index, v_next);
                    tag_e.remove(&e);
                    if v_next_index == 0 {
                        index.insert(v_next, vs_iter_next);
                        bm.vert_mut(v_next).e = Some(e);
                        lb_tmp.push(v_next);
                    } else if (dir < 0) == (v_next_index < 0) {
                        // Same side.
                    } else {
                        let m = if dir == 1 { (vs, v_next) } else { (v_next, vs) };
                        return (true, Some(m));
                    }
                }
            }
        }
        *lb = lb_tmp;
        (!lb.is_empty(), None)
    };
    loop {
        frap_math::budget::check();
        let (ok, m) = step(bm, &mut index, &mut tag_e, &mut lb_src, 1);
        if m.is_some() {
            v_match = m;
            break;
        }
        if !ok {
            break;
        }
        let (ok, m) = step(bm, &mut index, &mut tag_e, &mut lb_dst, -1);
        if m.is_some() {
            v_match = m;
            break;
        }
        if !ok {
            break;
        }
    }
    let (m0, m1) = v_match?;
    let mut verts: std::collections::VecDeque<VertId> = std::collections::VecDeque::new();
    let mut v = m0;
    loop {
        verts.push_front(v);
        if v == v_src {
            break;
        }
        let e = bm.vert(v).e.expect("path edge");
        v = bm.edge_other_vert(e, v);
    }
    let mut v = m1;
    loop {
        verts.push_back(v);
        if v == v_dst {
            break;
        }
        let e = bm.vert(v).e.expect("path edge");
        v = bm.edge_other_vert(e, v);
    }
    Some(EdgeLoopStore::from_verts(verts.into_iter().collect(), false))
}

/// `BM_mesh_edgeloops_calc_center`.
pub fn mesh_edgeloops_calc_center(bm: &BMesh, eloops: &mut [EdgeLoopStore]) {
    for el in eloops {
        el.calc_center(bm);
    }
}

/// `BM_mesh_edgeloops_calc_normal`.
pub fn mesh_edgeloops_calc_normal(bm: &BMesh, eloops: &mut [EdgeLoopStore]) {
    for el in eloops {
        el.calc_normal(bm);
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_edgeloop.cc
// (BM_mesh_edgeloops_calc_order).
/// `BM_mesh_edgeloops_calc_order`: from the outermost loop, each next loop is the closest
/// one (scaled by how much the loops face each other with `use_normals`).
pub fn mesh_edgeloops_calc_order(eloops: &mut Vec<EdgeLoopStore>, use_normals: bool) {
    let mut cent = [0.0f32; 3];
    let mut tot = 0;
    for el in eloops.iter() {
        if !bli::is_finite_v3(el.co) {
            continue;
        }
        cent = bli::add(cent, el.co);
        tot += 1;
    }
    if tot > 0 {
        cent = bli::scale(cent, 1.0 / tot as f32);
        if !bli::is_finite_v3(cent) {
            cent = [0.0; 3];
        }
    }
    let mut ordered = Vec::with_capacity(eloops.len());
    {
        let mut best: Option<usize> = None;
        let mut len_best_sq = -1.0f32;
        for (i, el) in eloops.iter().enumerate() {
            let l = bli::len_squared_v3v3(cent, el.co);
            if l > len_best_sq || best.is_none() {
                len_best_sq = l;
                best = Some(i);
            }
        }
        if let Some(b) = best {
            ordered.push(eloops.remove(b));
        }
    }
    while !eloops.is_empty() {
        let last: &EdgeLoopStore = ordered.last().expect("non-empty");
        let (co, no) = (last.co, last.no);
        let mut best: Option<usize> = None;
        let mut len_best_sq = f32::MAX;
        for (i, el) in eloops.iter().enumerate() {
            let len_sq = if use_normals {
                let (dir, l) = bli::normalize_v3(bli::sub(co, el.co));
                l * ((1.0 - bli::dot(dir, no).abs()) + (1.0 - bli::dot(dir, el.no).abs()))
            } else {
                bli::len_squared_v3v3(co, el.co)
            };
            if len_sq < len_best_sq || best.is_none() {
                len_best_sq = len_sq;
                best = Some(i);
            }
        }
        ordered.push(eloops.remove(best.expect("non-empty")));
    }
    *eloops = ordered;
}

/// `BM_edgeloop_overlap_check`: the two loops share a vertex.
pub fn edgeloop_overlap_check(a: &EdgeLoopStore, b: &EdgeLoopStore) -> bool {
    let (small, big) = if a.len() > b.len() { (b, a) } else { (a, b) };
    let set: FxHashSet<VertId> = small.verts.iter().copied().collect();
    let big_set: FxHashSet<VertId> = big.verts.iter().copied().collect();
    set.iter().any(|v| big_set.contains(v))
}
