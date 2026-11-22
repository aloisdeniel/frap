// SPDX-License-Identifier: GPL-3.0-or-later
//! Shortest paths and path regions between two UV elements (`BM_mesh_calc_path_uv_vert/edge/
//! face`, `BM_mesh_calc_path_uv_region_vert/edge/face`) and the min-heap they use.
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_path_uv.cc,
//! blender/source/blender/bmesh/tools/bmesh_path_region_uv.cc,
//! blender/source/blender/bmesh/intern/bmesh_query_uv.cc
//! (`BM_face_uv_calc_center_median_weighted`) and
//! blender/source/blender/blenlib/intern/BLI_heap_simple.cc, Blender 5.2.4,
//! GPL-2.0-or-later. The heap is ported node for node so equal costs pop in Blender's order.
//! Blender marks visited loops/faces with `BM_ELEM_TAG` and leaves the tags set; so does this
//! port (the region of faces reads the loop tags as Blender does, stale or not).

use frap_bmesh::{ElemFlag, FaceId, LoopId, VertId};

use super::{Uv, UvSel};

/// `HeapSimple`: a binary min-heap of `(value, element)`.
struct HeapSimple<T: Copy> {
    tree: Vec<(f32, T)>,
}

impl<T: Copy> HeapSimple<T> {
    fn new() -> Self {
        HeapSimple { tree: Vec::new() }
    }
    fn up(&mut self, mut i: usize, value: f32, ptr: T) {
        while i > 0 {
            let p = (i - 1) >> 1;
            if value >= self.tree[p].0 {
                break;
            }
            self.tree[i] = self.tree[p];
            i = p;
        }
        self.tree[i] = (value, ptr);
    }
    fn insert(&mut self, value: f32, ptr: T) {
        self.tree.push((value, ptr));
        let i = self.tree.len() - 1;
        self.up(i, value, ptr);
    }
    fn down(&mut self, start: usize, init: (f32, T)) {
        let size = self.tree.len();
        let (active_val, active_ptr) = init;
        let mut i = start;
        self.tree[i].0 = active_val;
        loop {
            let l = 2 * i + 1;
            let r = l + 1;
            let mut smallest = i;
            if l < size && self.tree[l].0 < active_val {
                smallest = l;
            }
            if r < size && self.tree[r].0 < self.tree[smallest].0 {
                smallest = r;
            }
            if smallest == i {
                break;
            }
            self.tree[i] = self.tree[smallest];
            i = smallest;
            self.tree[i].0 = active_val;
        }
        self.tree[i].1 = active_ptr;
    }
    fn pop_min(&mut self) -> Option<T> {
        let top = self.tree.first()?.1;
        let last = self.tree.pop().expect("non-empty");
        if !self.tree.is_empty() {
            self.down(0, last);
        }
        Some(top)
    }
}

/// `BMCalcPathUVParams`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct PathParams {
    pub use_topology_distance: bool,
    pub use_step_face: bool,
    pub aspect_y: f32,
}

fn sub2(a: Uv, b: Uv) -> Uv {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot2(a: Uv, b: Uv) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}
fn len2(a: Uv, b: Uv) -> f32 {
    let d = sub2(b, a);
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}
fn cross2(a: Uv, b: Uv) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}
/// `normalize_v2`: the unit vector (by the reciprocal) and the length.
fn normalize2(a: Uv) -> (Uv, f32) {
    let d = dot2(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let m = 1.0 / d;
        ([a[0] * m, a[1] * m], d)
    } else {
        ([0.0; 2], 0.0)
    }
}
fn mul2(a: Uv, b: Uv) -> Uv {
    [a[0] * b[0], a[1] * b[1]]
}

/// `step_cost_3_v2_ex`.
fn step_cost_3_v2_ex(v1: Uv, v2: Uv, v3: Uv, skip_12: bool, skip_23: bool) -> f32 {
    let (d1, cost_12) = normalize2(sub2(v2, v1));
    let (d2, cost_23) = normalize2(sub2(v3, v2));
    let cost = (if skip_12 { 0.0 } else { cost_12 }) + (if skip_23 { 0.0 } else { cost_23 });
    cost * (1.0 + 0.5 * (2.0 - dot2(d1, d2).abs().sqrt()))
}

/// `isect_line_line_v2_point` (`None` when colinear).
fn isect_line_line_v2_point(v0: Uv, v1: Uv, v2: Uv, v3: Uv) -> Option<Uv> {
    let s10 = sub2(v1, v0);
    let s32 = sub2(v3, v2);
    let div = cross2(s10, s32);
    if div != 0.0 {
        let u = cross2(v1, v0);
        let v = cross2(v3, v2);
        return Some([
            ((s32[0] * u) - (s10[0] * v)) / div,
            ((s32[1] * u) - (s10[1] * v)) / div,
        ]);
    }
    None
}

/// `line_point_factor_v2`.
fn line_point_factor_v2(p: Uv, l1: Uv, l2: Uv) -> f32 {
    let u = sub2(l2, l1);
    let h = sub2(p, l1);
    let d = dot2(u, u);
    if d > 0.0 { dot2(u, h) / d } else { 0.0 }
}

impl UvSel<'_> {
    /// `BM_face_uv_calc_center_median_weighted`.
    fn face_uv_center_median_weighted(&self, f: FaceId, aspect: Uv) -> Uv {
        let first = self.first_loop(f);
        let ua = |l: LoopId| mul2(self.uv(l), aspect);
        let mut cent = [0.0f32; 2];
        let mut totw = 0.0f32;
        let mut w_prev = len2(ua(self.prev(first)), ua(first));
        let mut uv_curr = ua(first);
        let mut l = first;
        loop {
            let uv_next = ua(self.next(l));
            let w_curr = len2(uv_curr, uv_next);
            let w = w_curr + w_prev;
            cent[0] += uv_curr[0] * w;
            cent[1] += uv_curr[1] * w;
            totw += w;
            w_prev = w_curr;
            uv_curr = uv_next;
            l = self.next(l);
            if l == first {
                break;
            }
        }
        if totw != 0.0 {
            let m = 1.0 / totw;
            cent = [cent[0] * m, cent[1] * m];
        }
        [cent[0] / aspect[0], cent[1] / aspect[1]]
    }

    /// Loop indices in face order (`BM_elem_index_set` of the path functions); returns
    /// `(index by slot, loops by index)`.
    fn loop_table(&self) -> (Vec<usize>, Vec<LoopId>) {
        let mut idx = vec![usize::MAX; self.bm.loops.capacity()];
        let mut loops = Vec::new();
        for f in self.faces() {
            for l in self.face_loops(f) {
                idx[l.slot()] = loops.len();
                loops.push(l);
            }
        }
        (idx, loops)
    }

    /// Sets `BM_ELEM_TAG` of every loop to `!filter(l)` (the face is visible).
    fn tag_loops_by_visibility(&mut self) {
        for f in self.faces() {
            let hidden = !self.face_visible(f);
            for l in self.face_loops(f) {
                self.set_lflag(l, ElemFlag::TAG, hidden);
            }
        }
    }

    // ------------------------------------------------------------------- vert path

    /// `BM_mesh_calc_path_uv_vert` (filter: the face is visible).
    pub(crate) fn path_uv_vert(&mut self, src: LoopId, dst: LoopId, p: PathParams) -> Vec<LoopId> {
        self.tag_loops_by_visibility();
        let (idx, _) = self.loop_table();
        let n = idx.iter().filter(|&&i| i != usize::MAX).count();
        let mut prev: Vec<Option<LoopId>> = vec![None; n];
        let mut cost = vec![f32::MAX; n];
        let mut heap = HeapSimple::new();
        heap.insert(0.0, src);
        cost[idx[src.slot()]] = 0.0;
        let mut l = src;
        let is_dst = |s: &Self, l: LoopId| s.lv(l) == s.lv(dst) && s.share_vert(l, dst);
        while let Some(x) = heap.pop_min() {
            frap_math::budget::check();
            l = x;
            if is_dst(self, l) {
                break;
            }
            if !self.lflag(l, ElemFlag::TAG) {
                self.set_lflag(l, ElemFlag::TAG, true);
                self.verttag_add_adjacent_uv(&mut heap, l, &mut prev, &mut cost, &idx, p);
            }
        }
        let mut path = Vec::new();
        if is_dst(self, l) {
            let mut cur = Some(l);
            while let Some(c) = cur {
                path.push(c);
                cur = prev[idx[c.slot()]];
            }
            path.reverse();
        }
        path
    }

    fn verttag_add_adjacent_uv(
        &mut self,
        heap: &mut HeapSimple<LoopId>,
        l_a: LoopId,
        prev: &mut [Option<LoopId>],
        cost: &mut [f32],
        idx: &[usize],
        p: PathParams,
    ) {
        let l_a_index = idx[l_a.slot()];
        let luv_a = self.uv(l_a);
        let uv_a = [luv_a[0], luv_a[1] / p.aspect_y];
        for l in self.bm.vert_loops(self.lv(l_a)) {
            if self.uv(l) != luv_a {
                continue;
            }
            self.set_lflag(l, ElemFlag::TAG, true);
            let mut l_b = self.next(l);
            loop {
                if !self.lflag(l_b, ElemFlag::TAG) {
                    let luv_b = self.uv(l_b);
                    let uv_b = [luv_b[0], luv_b[1] / p.aspect_y];
                    let l_b_index = idx[l_b.slot()];
                    let cost_cut = if p.use_topology_distance {
                        1.0
                    } else {
                        len2(uv_a, uv_b)
                    };
                    let cost_new = cost[l_a_index] + cost_cut;
                    if cost[l_b_index] > cost_new {
                        cost[l_b_index] = cost_new;
                        prev[l_b_index] = Some(l_a);
                        heap.insert(cost_new, l_b);
                    }
                }
                if !p.use_step_face && l_b == self.next(l) {
                    l_b = self.prev(self.prev(l));
                }
                l_b = self.next(l_b);
                if l_b == l {
                    break;
                }
            }
        }
    }

    // ------------------------------------------------------------------- edge path

    /// `edgetag_cut_cost_vert_uv`.
    fn edgetag_cut_cost_vert_uv(
        &self,
        l_e_a: LoopId,
        l_e_b: LoopId,
        l_v: LoopId,
        aspect_y: f32,
    ) -> f32 {
        let l_v1 = if self.lv(l_v) == self.lv(l_e_a) {
            self.next(l_e_a)
        } else {
            l_e_a
        };
        let l_v2 = if self.lv(l_v) == self.lv(l_e_b) {
            self.next(l_e_b)
        } else {
            l_e_b
        };
        let a = |l: LoopId| {
            let u = self.uv(l);
            [u[0], u[1] / aspect_y]
        };
        step_cost_3_v2_ex(a(l_v1), a(l_v), a(l_v2), false, false)
    }

    /// `edgetag_cut_cost_face_uv` (Blender takes the "centre" of each edge's first corner
    /// alone: `mid_v2_v2v2(c, luv, luv)`).
    fn edgetag_cut_cost_face_uv(&self, l_e_a: LoopId, l_e_b: LoopId, f: FaceId, aspect: Uv) -> f32 {
        let a_cent = mul2(self.uv(l_e_a), aspect);
        let b_cent = mul2(self.uv(l_e_b), aspect);
        let f_cent = self.face_uv_center_median_weighted(f, aspect);
        step_cost_3_v2_ex(a_cent, b_cent, f_cent, false, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn edgetag_add_adjacent_uv(
        &mut self,
        heap: &mut HeapSimple<LoopId>,
        l_a: LoopId,
        prev: &mut [Option<LoopId>],
        cost: &mut [f32],
        idx: &[usize],
        p: PathParams,
    ) {
        let l_a_index = idx[l_a.slot()];
        let l_a_verts = [l_a, self.next(l_a)];
        if !p.use_step_face {
            for &lv in &l_a_verts {
                if let Some(mut l_prev) = prev[l_a_index] {
                    let mut ok = true;
                    if self.lv(lv) != self.lv(l_prev) {
                        if self.lv(lv) == self.lv(self.next(l_prev)) {
                            l_prev = self.next(l_prev);
                        } else {
                            ok = false;
                        }
                    }
                    if ok && self.share_vert(lv, l_prev) {
                        continue;
                    }
                }
                for e_b in self.bm.vert_edges(self.lv(lv)) {
                    let Some(first) = self.bm.edge(e_b).l else {
                        continue;
                    };
                    let mut l_b = first;
                    loop {
                        if !self.lflag(l_b, ElemFlag::TAG) {
                            let l_b_vert = if self.lv(lv) == self.lv(l_b) {
                                l_b
                            } else {
                                self.next(l_b)
                            };
                            if self.share_vert(lv, l_b_vert) {
                                let l_b_index = idx[l_b.slot()];
                                let cost_cut = if p.use_topology_distance {
                                    1.0
                                } else {
                                    self.edgetag_cut_cost_vert_uv(l_a, l_b, lv, p.aspect_y)
                                };
                                let cost_new = cost[l_a_index] + cost_cut;
                                if cost[l_b_index] > cost_new {
                                    cost[l_b_index] = cost_new;
                                    prev[l_b_index] = Some(l_a);
                                    heap.insert(cost_new, l_b);
                                }
                            }
                        }
                        l_b = self.radial_next(l_b);
                        if l_b == first {
                            break;
                        }
                    }
                }
            }
        } else {
            let aspect = [1.0, 1.0 / p.aspect_y];
            let mut l_iter = l_a;
            loop {
                if self.share_edge(l_a, l_iter) {
                    let l_cycle_end = l_iter;
                    let mut l_b = self.next(l_iter);
                    loop {
                        if !self.lflag(l_b, ElemFlag::TAG) {
                            let l_b_index = idx[l_b.slot()];
                            let cost_cut = if p.use_topology_distance {
                                1.0
                            } else {
                                self.edgetag_cut_cost_face_uv(l_a, l_b, self.lf(l_iter), aspect)
                            };
                            let cost_new = cost[l_a_index] + cost_cut;
                            if cost[l_b_index] > cost_new {
                                cost[l_b_index] = cost_new;
                                prev[l_b_index] = Some(l_a);
                                heap.insert(cost_new, l_b);
                            }
                        }
                        l_b = self.next(l_b);
                        if l_b == l_cycle_end {
                            break;
                        }
                    }
                }
                l_iter = self.radial_next(l_iter);
                if l_iter == l_a {
                    break;
                }
            }
        }
    }

    /// `BM_mesh_calc_path_uv_edge` (filter: the face is visible).
    pub(crate) fn path_uv_edge(&mut self, src: LoopId, dst: LoopId, p: PathParams) -> Vec<LoopId> {
        self.tag_loops_by_visibility();
        let (idx, _) = self.loop_table();
        let n = idx.iter().filter(|&&i| i != usize::MAX).count();
        let mut prev: Vec<Option<LoopId>> = vec![None; n];
        let mut cost = vec![f32::MAX; n];
        let mut heap = HeapSimple::new();
        heap.insert(0.0, src);
        cost[idx[src.slot()]] = 0.0;
        let mut l = src;
        let is_dst = |s: &Self, l: LoopId| s.le(l) == s.le(dst) && s.share_edge(l, dst);
        while let Some(x) = heap.pop_min() {
            frap_math::budget::check();
            l = x;
            if is_dst(self, l) {
                break;
            }
            if !self.lflag(l, ElemFlag::TAG) {
                self.set_lflag(l, ElemFlag::TAG, true);
                self.edgetag_add_adjacent_uv(&mut heap, l, &mut prev, &mut cost, &idx, p);
            }
        }
        let mut path = Vec::new();
        if is_dst(self, l) {
            let mut cur = Some(l);
            while let Some(c) = cur {
                path.push(c);
                cur = prev[idx[c.slot()]];
            }
            path.reverse();
        }
        path
    }

    // ------------------------------------------------------------------- face path

    fn facetag_cut_cost_edge_uv(
        &self,
        f_a: FaceId,
        f_b: FaceId,
        l_edge: LoopId,
        ends: [FaceId; 2],
        aspect: Uv,
    ) -> f32 {
        let mut a_cent = self.face_uv_center_median_weighted(f_a, aspect);
        let mut b_cent = self.face_uv_center_median_weighted(f_b, aspect);
        let co_v1 = self.uv(l_edge);
        let co_v2 = self.uv(self.next(l_edge));
        // `ix_e` is left unset by Blender for parallel lines; frap takes the first point.
        let ix_e = isect_line_line_v2_point(co_v1, co_v2, a_cent, b_cent).unwrap_or(co_v1);
        let factor = line_point_factor_v2(ix_e, co_v1, co_v2);
        let mut e_cent = if factor < 0.0 {
            co_v1
        } else if factor > 1.0 {
            co_v2
        } else {
            ix_e
        };
        a_cent = mul2(a_cent, aspect);
        b_cent = mul2(b_cent, aspect);
        e_cent = mul2(e_cent, aspect);
        step_cost_3_v2_ex(a_cent, e_cent, b_cent, f_a == ends[0], f_b == ends[1])
    }

    fn facetag_cut_cost_vert_uv(
        &self,
        f_a: FaceId,
        f_b: FaceId,
        l_vert: LoopId,
        ends: [FaceId; 2],
        aspect: Uv,
    ) -> f32 {
        let a_cent = mul2(self.face_uv_center_median_weighted(f_a, aspect), aspect);
        let b_cent = mul2(self.face_uv_center_median_weighted(f_b, aspect), aspect);
        let v_cent = mul2(self.uv(l_vert), aspect);
        step_cost_3_v2_ex(a_cent, v_cent, b_cent, f_a == ends[0], f_b == ends[1])
    }

    /// `BM_loop_share_edge_check`.
    fn loop_share_edge_check(&self, a: LoopId, b: LoopId) -> bool {
        let (ae, be) = (self.le(a), self.le(b));
        let (ape, bpe) = (self.le(self.prev(a)), self.le(self.prev(b)));
        ae == be || ae == bpe || be == ae || be == ape
    }

    /// `BM_mesh_calc_path_uv_face` (filter: the face is visible).
    pub(crate) fn path_uv_face(&mut self, src: FaceId, dst: FaceId, p: PathParams) -> Vec<FaceId> {
        let aspect = [1.0, 1.0 / p.aspect_y];
        let ends = [src, dst];
        let faces = self.faces();
        let mut fidx = vec![usize::MAX; self.bm.faces.capacity()];
        for (i, &f) in faces.iter().enumerate() {
            let hidden = !self.face_visible(f);
            self.set_fflag(f, ElemFlag::TAG, hidden);
            fidx[f.slot()] = i;
        }
        let n = faces.len();
        let mut prev: Vec<Option<FaceId>> = vec![None; n];
        let mut cost = vec![f32::MAX; n];
        let mut heap = HeapSimple::new();
        heap.insert(0.0, src);
        cost[fidx[src.slot()]] = 0.0;
        let mut f = src;
        while let Some(x) = heap.pop_min() {
            frap_math::budget::check();
            f = x;
            if f == dst {
                break;
            }
            if !self.fflag(f, ElemFlag::TAG) {
                self.set_fflag(f, ElemFlag::TAG, true);
                let f_a = f;
                let f_a_index = fidx[f_a.slot()];
                let mut relax = |s: &Self, f_b: FaceId, c: f32, heap: &mut HeapSimple<FaceId>| {
                    let f_b_index = fidx[f_b.slot()];
                    let cost_new = cost[f_a_index] + c;
                    if cost[f_b_index] > cost_new {
                        cost[f_b_index] = cost_new;
                        prev[f_b_index] = Some(f_a);
                        heap.insert(cost_new, f_b);
                    }
                    let _ = s;
                };
                for l_a in self.face_loops(f_a) {
                    if l_a == self.radial_next(l_a) {
                        continue;
                    }
                    let mut l_iter = self.radial_next(l_a);
                    let first = l_iter;
                    loop {
                        let f_b = self.lf(l_iter);
                        if !self.fflag(f_b, ElemFlag::TAG) && self.share_edge(l_a, l_iter) {
                            let c = if p.use_topology_distance {
                                1.0
                            } else {
                                self.facetag_cut_cost_edge_uv(f_a, f_b, l_iter, ends, aspect)
                            };
                            relax(self, f_b, c, &mut heap);
                        }
                        l_iter = self.radial_next(l_iter);
                        if l_iter == first {
                            break;
                        }
                    }
                }
                if p.use_step_face {
                    for l_a in self.face_loops(f_a) {
                        for l_b in self.bm.vert_loops(self.lv(l_a)) {
                            if l_a == l_b || self.loop_share_edge_check(l_a, l_b) {
                                continue;
                            }
                            let f_b = self.lf(l_b);
                            if !self.fflag(f_b, ElemFlag::TAG) && self.share_vert(l_a, l_b) {
                                let c = if p.use_topology_distance {
                                    1.0
                                } else {
                                    self.facetag_cut_cost_vert_uv(f_a, f_b, l_a, ends, aspect)
                                };
                                relax(self, f_b, c, &mut heap);
                            }
                        }
                    }
                }
            }
        }
        let mut path = Vec::new();
        if f == dst {
            let mut cur = Some(f);
            while let Some(c) = cur {
                path.push(c);
                cur = prev[fidx[c.slot()]];
            }
            path.reverse();
        }
        path
    }

    // ---------------------------------------------------------------- path regions

    /// `BM_vert_is_edge_pair_manifold`.
    fn vert_is_edge_pair_manifold(&self, v: VertId) -> bool {
        let Some(e) = self.bm.vert(v).e else {
            return false;
        };
        let e_other = self.bm.disk_edge_next(e, v);
        e_other != e
            && self.bm.disk_edge_next(e_other, v) == e
            && self.bm.edge_is_manifold(e)
            && self.bm.edge_is_manifold(e_other)
    }

    /// `bm_loop_pair_ends`.
    fn loop_pair_ends(&self, l_pivot: LoopId) -> Option<[LoopId; 2]> {
        let mut ends = [l_pivot; 2];
        for (j, end) in ends.iter_mut().enumerate() {
            let step = |l: LoopId| if j == 1 { self.next(l) } else { self.prev(l) };
            let mut l_other = step(l_pivot);
            while self.vert_is_edge_pair_manifold(self.lv(l_other)) {
                l_other = step(l_other);
                if l_other == l_pivot {
                    return None;
                }
            }
            *end = l_other;
        }
        Some(ends)
    }

    fn region_test(&self, l: LoopId, depths: &[Vec<i32>; 2], idx: &[usize], pass: i32) -> bool {
        let i = idx[l.slot()];
        depths[0][i] != -1 && depths[1][i] != -1 && (depths[0][i] + depths[1][i]) < pass
    }

    fn region_test_chain(
        &self,
        l: LoopId,
        depths: &[Vec<i32>; 2],
        idx: &[usize],
        pass: i32,
    ) -> bool {
        if self.region_test(l, depths, idx, pass) {
            return true;
        }
        if self.vert_is_edge_pair_manifold(self.lv(l))
            && let Some(ends) = self.loop_pair_ends(l)
            && self.region_test(ends[0], depths, idx, pass)
            && self.region_test(ends[1], depths, idx, pass)
        {
            return true;
        }
        false
    }

    /// `mesh_calc_path_region_elem`: the region's loops (`Region::Loops`, vertex or edge
    /// paths) or faces, in Blender's prepend order.
    fn path_region_elem(&mut self, src: BMElem2, dst: BMElem2, path_htype: Htype) -> Region {
        let ele_loops: [Vec<LoopId>; 2] = [src, dst].map(|e| match e {
            BMElem2::Face(f) => self.face_loops(f),
            BMElem2::Loop(l) => match path_htype {
                Htype::Edge => vec![l, self.next(l)],
                _ => vec![l],
            },
        });
        let (idx, loops) = self.loop_table();
        let n = loops.len();
        let mut depths: [Vec<i32>; 2] = [Vec::new(), Vec::new()];
        let mut pass = 0i32;
        let mut found_all = false;
        for side in 0..2 {
            let side_other = 1 - side;
            depths[side] = vec![-1; n];
            let mut stack: Vec<LoopId> = Vec::new();
            let mut stack_other: Vec<LoopId> = Vec::new();
            for &l in &ele_loops[side] {
                depths[side][idx[l.slot()]] = 0;
                if !self.lflag(l, ElemFlag::TAG) {
                    stack.push(l);
                }
            }
            if matches!(path_htype, Htype::Vert | Htype::Edge) {
                for &l in &ele_loops[side] {
                    if self.vert_is_edge_pair_manifold(self.lv(l))
                        && let Some(ends) = self.loop_pair_ends(l)
                    {
                        for e in ends {
                            let ei = idx[e.slot()];
                            if depths[side][ei] == -1 {
                                depths[side][ei] = 0;
                                if !self.lflag(e, ElemFlag::TAG) {
                                    stack.push(e);
                                }
                            }
                        }
                    }
                }
            }
            found_all = false;
            pass = 1;
            while !stack.is_empty() {
                while let Some(l_a) = stack.pop() {
                    frap_math::budget::check();
                    let l_a_index = idx[l_a.slot()];
                    for l_iter in self.bm.vert_loops(self.lv(l_a)) {
                        if self.lflag(l_iter, ElemFlag::TAG) || !self.share_vert(l_a, l_iter) {
                            continue;
                        }
                        let ii = idx[l_iter.slot()];
                        if depths[side][ii] == -1 {
                            depths[side][ii] = depths[side][l_a_index];
                        }
                        for j in 0..2 {
                            let step = |l: LoopId| if j == 1 { self.next(l) } else { self.prev(l) };
                            let back = if j == 1 {
                                self.prev(l_iter)
                            } else {
                                self.next(l_iter)
                            };
                            let mut l_b = step(l_iter);
                            let mut l_b_index = idx[l_b.slot()];
                            if depths[side][l_b_index] == -1 {
                                while self.vert_is_edge_pair_manifold(self.lv(l_b))
                                    && depths[side][l_b_index] == -1
                                    && l_b != back
                                {
                                    depths[side][l_b_index] = pass;
                                    l_b = step(l_b);
                                    l_b_index = idx[l_b.slot()];
                                }
                                if depths[side][l_b_index] == -1 {
                                    depths[side][l_b_index] = pass;
                                    if !self.lflag(l_b, ElemFlag::TAG) {
                                        stack_other.push(l_b);
                                    }
                                }
                            }
                        }
                    }
                }
                found_all = ele_loops[side_other]
                    .iter()
                    .all(|l| depths[side][idx[l.slot()]] != -1);
                if found_all {
                    pass += 1;
                    break;
                }
                std::mem::swap(&mut stack, &mut stack_other);
                pass += 1;
            }
            if !found_all {
                break;
            }
        }
        if !found_all {
            return Region::Loops(Vec::new());
        }
        match path_htype {
            Htype::Face => {
                let mut out = Vec::new();
                for f in self.faces() {
                    if self.fflag(f, ElemFlag::TAG) {
                        continue;
                    }
                    let mut ok_tests = if self.bm.face(f).len > 3 { 1 } else { 0 };
                    let mut ok = true;
                    for l in self.face_loops(f) {
                        if !self.region_test_chain(l, &depths, &idx, pass) {
                            if ok_tests == 0 {
                                ok = false;
                                break;
                            }
                            ok_tests -= 1;
                        }
                    }
                    if ok {
                        out.push(f);
                    }
                }
                out.reverse();
                Region::Faces(out)
            }
            Htype::Edge => {
                let mut out = Vec::new();
                for f in self.faces() {
                    let first = self.first_loop(f);
                    let mut next_in = self.region_test_chain(first, &depths, &idx, pass);
                    for l in self.face_loops(f) {
                        let cur_in = next_in;
                        next_in = self.region_test_chain(self.next(l), &depths, &idx, pass);
                        if cur_in && next_in {
                            out.push(l);
                        }
                    }
                }
                out.reverse();
                Region::Loops(out)
            }
            Htype::Vert => {
                let mut out = Vec::new();
                for f in self.faces() {
                    for l in self.face_loops(f) {
                        if self.region_test_chain(l, &depths, &idx, pass) {
                            out.push(l);
                        }
                    }
                }
                out.reverse();
                Region::Loops(out)
            }
        }
    }

    /// `BM_mesh_calc_path_uv_region_vert` / `_edge` (filter: the face is visible).
    pub(crate) fn path_uv_region_loops(
        &mut self,
        src: LoopId,
        dst: LoopId,
        edge: bool,
    ) -> Vec<LoopId> {
        self.tag_loops_by_visibility();
        let ht = if edge { Htype::Edge } else { Htype::Vert };
        match self.path_region_elem(BMElem2::Loop(src), BMElem2::Loop(dst), ht) {
            Region::Loops(v) => v,
            Region::Faces(_) => Vec::new(),
        }
    }

    /// `BM_mesh_calc_path_uv_region_face` (filter: the face is visible; the vertex tags are
    /// set as Blender does, the loop tags are left as they are).
    pub(crate) fn path_uv_region_face(&mut self, src: FaceId, dst: FaceId) -> Vec<FaceId> {
        for v in self.bm.vert_ids() {
            self.bm.vert_mut(v).flag.insert(ElemFlag::TAG);
        }
        for f in self.faces() {
            let test = !self.face_visible(f);
            self.set_fflag(f, ElemFlag::TAG, test);
            if !test {
                for l in self.face_loops(f) {
                    let v = self.lv(l);
                    self.bm.vert_mut(v).flag.remove(ElemFlag::TAG);
                }
            }
        }
        match self.path_region_elem(BMElem2::Face(src), BMElem2::Face(dst), Htype::Face) {
            Region::Faces(v) => v,
            Region::Loops(_) => Vec::new(),
        }
    }
}

/// A path endpoint (`BMElem *` of type `BM_LOOP` or `BM_FACE`).
#[derive(Copy, Clone, Debug)]
enum BMElem2 {
    Loop(LoopId),
    Face(FaceId),
}

/// `path_htype`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Htype {
    Vert,
    Edge,
    Face,
}

/// A path region's elements.
enum Region {
    Loops(Vec<LoopId>),
    Faces(Vec<FaceId>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_pops_ties_in_insertion_shape() {
        let mut h = HeapSimple::new();
        for (v, x) in [(2.0, 'a'), (1.0, 'b'), (1.0, 'c'), (0.5, 'd'), (3.0, 'e')] {
            h.insert(v, x);
        }
        let mut out = Vec::new();
        while let Some(x) = h.pop_min() {
            out.push(x);
        }
        assert_eq!(out.first(), Some(&'d'));
        assert_eq!(out.last(), Some(&'e'));
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn step_cost_prefers_straight_paths() {
        let straight = step_cost_3_v2_ex([0.0, 0.0], [1.0, 0.0], [2.0, 0.0], false, false);
        let turn = step_cost_3_v2_ex([0.0, 0.0], [1.0, 0.0], [1.0, 1.0], false, false);
        assert!(straight < turn);
        assert_eq!(
            isect_line_line_v2_point([0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]),
            None
        );
    }
}
