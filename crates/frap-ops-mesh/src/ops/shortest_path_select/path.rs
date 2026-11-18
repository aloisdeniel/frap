// SPDX-License-Identifier: GPL-3.0-or-later
//! Shortest paths and path regions between two elements (`BM_mesh_calc_path_vert/edge/face`,
//! `BM_mesh_calc_path_region_vert/edge/face`) and the min-heap they use (`HeapSimple`).
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_path.cc,
//! blender/source/blender/bmesh/tools/bmesh_path_region.cc and
//! blender/source/blender/blenlib/intern/BLI_heap_simple.cc (Blender 5.2.4,
//! GPL-2.0-or-later). The heap is ported node for node so that equal costs pop in Blender's
//! order. Elements Blender tags as visited (`BM_ELEM_TAG`) are tracked in local arrays here:
//! the BMesh is only read.

use frap_bmesh::query::geom::{V3, cross, dot, normalize_v3, scale, sub};
use frap_bmesh::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};

// --------------------------------------------------------------------------- HeapSimple

/// `HeapSimple`: a binary min-heap of `(value, element)`.
pub(crate) struct HeapSimple<T: Copy> {
    tree: Vec<(f32, T)>,
}

impl<T: Copy> HeapSimple<T> {
    pub(crate) fn new() -> Self {
        HeapSimple { tree: Vec::new() }
    }
    /// `heapsimple_up`.
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
    /// `BLI_heapsimple_insert`.
    pub(crate) fn insert(&mut self, value: f32, ptr: T) {
        self.tree.push((value, ptr));
        let i = self.tree.len() - 1;
        self.up(i, value, ptr);
    }
    /// `heapsimple_down` with the node `init` placed at `start`.
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
    /// `BLI_heapsimple_pop_min`.
    pub(crate) fn pop_min(&mut self) -> Option<T> {
        let top = self.tree.first()?.1;
        let last = self.tree.pop().expect("non-empty");
        if !self.tree.is_empty() {
            self.down(0, last);
        }
        Some(top)
    }
}

// -------------------------------------------------------------------------------- helpers

/// `BMCalcPathParams`.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct PathParams {
    pub(crate) use_topology_distance: bool,
    pub(crate) use_step_face: bool,
}

fn co(bm: &BMesh, v: VertId) -> V3 {
    bm.vert(v).co
}

/// `len_v3v3`.
fn len_v3v3(a: V3, b: V3) -> f32 {
    let d = sub(b, a);
    dot(d, d).sqrt()
}

/// `step_cost_3_v3_ex`.
fn step_cost_3_v3_ex(v1: V3, v2: V3, v3: V3, skip_12: bool, skip_23: bool) -> f32 {
    let (d1, cost_12) = normalize_v3(sub(v2, v1));
    let (d2, cost_23) = normalize_v3(sub(v3, v2));
    let cost = (if skip_12 { 0.0 } else { cost_12 }) + (if skip_23 { 0.0 } else { cost_23 });
    // Biased to give higher values to sharp turns.
    cost * (1.0 + 0.5 * (2.0 - dot(d1, d2).abs().sqrt()))
}

/// `BM_face_calc_center_median_weighted`.
fn face_center_median_weighted(bm: &BMesh, f: FaceId) -> V3 {
    let loops = bm.face_loops(f);
    let edge_len = |l: LoopId| {
        let [a, b] = bm.edge(bm.lp(l).e).v;
        len_v3v3(co(bm, a), co(bm, b))
    };
    let mut cent = [0.0f32; 3];
    let mut totw = 0.0f32;
    let mut w_prev = edge_len(bm.lp(loops[0]).prev);
    for &l in &loops {
        let w_curr = edge_len(l);
        let w = w_curr + w_prev;
        let c = co(bm, bm.lp(l).v);
        cent[0] += c[0] * w;
        cent[1] += c[1] * w;
        cent[2] += c[2] * w;
        totw += w;
        w_prev = w_curr;
    }
    if totw != 0.0 {
        cent = scale(cent, 1.0 / totw);
    }
    cent
}

/// `project_v3_v3v3`.
fn project_v3_v3v3(p: V3, v_proj: V3) -> V3 {
    if v_proj == [0.0; 3] {
        return [0.0; 3];
    }
    let mul = dot(p, v_proj) / dot(v_proj, v_proj);
    scale(v_proj, mul)
}

/// `isect_line_line_v3` (`isect_line_line_epsilon_v3` with `epsilon = 1e-6`): the point on the
/// first line, or `None` for a zero-length line.
fn isect_line_line_v3(v1: V3, v2: V3, v3: V3, v4: V3) -> Option<V3> {
    let epsilon = 0.000_001f32;
    let c = sub(v3, v1);
    let a = sub(v2, v1);
    let b = sub(v4, v3);
    let ab = cross(a, b);
    let d = dot(c, ab);
    let div = dot(ab, ab);
    if div == 0.0 {
        return None;
    }
    if d.abs() <= epsilon {
        let cb = cross(c, b);
        let a = scale(a, dot(cb, ab) / div);
        return Some([v1[0] + a[0], v1[1] + a[1], v1[2] + a[2]]);
    }
    let t = sub(v1, v3);
    let n = cross(a, b);
    let t = project_v3_v3v3(t, n);
    let v3t = [v3[0] + t[0], v3[1] + t[1], v3[2] + t[2]];
    let v4t = [v4[0] + t[0], v4[1] + t[1], v4[2] + t[2]];
    let c = sub(v3t, v1);
    let a = sub(v2, v1);
    let b = sub(v4t, v3t);
    let ab = cross(a, b);
    let cb = cross(c, b);
    let a = scale(a, dot(cb, ab) / dot(ab, ab));
    Some([v1[0] + a[0], v1[1] + a[1], v1[2] + a[2]])
}

/// `line_point_factor_v3`.
fn line_point_factor_v3(p: V3, l1: V3, l2: V3) -> f32 {
    let u = sub(l2, l1);
    let h = sub(p, l1);
    let d = dot(u, u);
    if d > 0.0 { dot(u, h) / d } else { 0.0 }
}

/// Walks `prev` back from `dst` (`BLI_linklist_prepend`): the path from source to `dst`.
fn unwind<T: Copy>(dst: T, slot: impl Fn(T) -> usize, prev: &[Option<T>]) -> Vec<T> {
    let mut path = vec![dst];
    let mut cur = dst;
    while let Some(p) = prev[slot(cur)] {
        path.push(p);
        cur = p;
    }
    path.reverse();
    path
}

// ----------------------------------------------------------------------------- vert path

/// `BM_mesh_calc_path_vert` (hidden vertices are not walked; `filter_fn` = not hidden).
pub(crate) fn path_vert(
    bm: &BMesh,
    src: VertId,
    dst: VertId,
    p: PathParams,
) -> Option<Vec<VertId>> {
    let cap = bm.verts.ids().map(|s| s as usize + 1).max().unwrap_or(0);
    let mut tag = vec![false; cap];
    for v in bm.vert_ids() {
        tag[v.slot()] = bm.vert(v).flag.contains(ElemFlag::HIDDEN);
    }
    let mut prev: Vec<Option<VertId>> = vec![None; cap];
    let mut cost = vec![f32::MAX; cap];
    let mut heap = HeapSimple::new();
    heap.insert(0.0, src);
    cost[src.slot()] = 0.0;
    let mut v = src;
    while let Some(x) = heap.pop_min() {
        v = x;
        if v == dst {
            break;
        }
        if tag[v.slot()] {
            continue;
        }
        tag[v.slot()] = true;
        // `verttag_add_adjacent`.
        let mut relax = |v_b: VertId, heap: &mut HeapSimple<VertId>| {
            if tag[v_b.slot()] {
                return;
            }
            let cut = if p.use_topology_distance {
                1.0
            } else {
                len_v3v3(co(bm, v), co(bm, v_b))
            };
            let new = cost[v.slot()] + cut;
            if cost[v_b.slot()] > new {
                cost[v_b.slot()] = new;
                prev[v_b.slot()] = Some(v);
                heap.insert(new, v_b);
            }
        };
        for e in bm.vert_edges(v) {
            relax(bm.edge_other_vert(e, v), &mut heap);
        }
        if p.use_step_face {
            for l in bm.vert_loops(v) {
                if bm.face(bm.lp(l).f).len <= 3 {
                    continue;
                }
                // Skip loops on adjacent edges.
                let stop = bm.lp(l).prev;
                let mut it = bm.lp(bm.lp(l).next).next;
                while it != stop {
                    relax(bm.lp(it).v, &mut heap);
                    it = bm.lp(it).next;
                }
            }
        }
    }
    (v == dst).then(|| unwind(dst, VertId::slot, &prev))
}

// ----------------------------------------------------------------------------- edge path

/// `BM_mesh_calc_path_edge`.
pub(crate) fn path_edge(
    bm: &BMesh,
    src: EdgeId,
    dst: EdgeId,
    p: PathParams,
) -> Option<Vec<EdgeId>> {
    let vcap = bm.verts.ids().map(|s| s as usize + 1).max().unwrap_or(0);
    let cap = bm.edges.ids().map(|s| s as usize + 1).max().unwrap_or(0);
    let mut vtag = vec![false; vcap];
    let mut tag = vec![false; cap];
    for e in bm.edge_ids() {
        tag[e.slot()] = bm.edge(e).flag.contains(ElemFlag::HIDDEN);
    }
    let mut prev: Vec<Option<EdgeId>> = vec![None; cap];
    let mut cost = vec![f32::MAX; cap];
    let mut heap = HeapSimple::new();
    heap.insert(0.0, src);
    cost[src.slot()] = 0.0;
    let mut e = src;
    while let Some(x) = heap.pop_min() {
        e = x;
        if e == dst {
            break;
        }
        if tag[e.slot()] {
            continue;
        }
        tag[e.slot()] = true;
        // Prevent the path overlapping itself in rare cases (#137456).
        for v in bm.edge(e).v {
            vtag[v.slot()] = true;
        }
        // `edgetag_add_adjacent`.
        let e_a = e;
        if !p.use_step_face || bm.edge(e_a).l.is_none() {
            for v in bm.edge(e_a).v {
                // Don't walk over the previous vertex.
                if let Some(pe) = prev[e_a.slot()]
                    && bm.vert_in_edge(pe, v)
                {
                    continue;
                }
                for e_b in bm.vert_edges(v) {
                    if tag[e_b.slot()] || vtag[bm.edge_other_vert(e_b, v).slot()] {
                        continue;
                    }
                    let cut = if p.use_topology_distance {
                        1.0
                    } else {
                        // `edgetag_cut_cost_vert`.
                        let v1 = bm.edge_other_vert(e_a, v);
                        let v2 = bm.edge_other_vert(e_b, v);
                        step_cost_3_v3_ex(co(bm, v1), co(bm, v), co(bm, v2), false, false)
                    };
                    let new = cost[e_a.slot()] + cut;
                    if cost[e_b.slot()] > new {
                        cost[e_b.slot()] = new;
                        prev[e_b.slot()] = Some(e_a);
                        heap.insert(new, e_b);
                    }
                }
            }
        } else {
            let l_first = bm.edge(e_a).l.expect("checked");
            let mut l_iter = l_first;
            loop {
                let mut it = bm.lp(l_iter).next;
                let end = l_iter;
                while it != end {
                    let e_b = bm.lp(it).e;
                    if !tag[e_b.slot()] {
                        let cut = if p.use_topology_distance {
                            1.0
                        } else {
                            // `edgetag_cut_cost_face`: Blender takes the midpoint of
                            // `v1` and `v1` of each edge (that is, `v1`).
                            let a = co(bm, bm.edge(e_a).v[0]);
                            let b = co(bm, bm.edge(e_b).v[0]);
                            let f = face_center_median_weighted(bm, bm.lp(l_iter).f);
                            step_cost_3_v3_ex(a, b, f, false, false)
                        };
                        let new = cost[e_a.slot()] + cut;
                        if cost[e_b.slot()] > new {
                            cost[e_b.slot()] = new;
                            prev[e_b.slot()] = Some(e_a);
                            heap.insert(new, e_b);
                        }
                    }
                    it = bm.lp(it).next;
                }
                l_iter = bm.lp(l_iter).radial_next;
                if l_iter == l_first {
                    break;
                }
            }
        }
    }
    (e == dst).then(|| unwind(dst, EdgeId::slot, &prev))
}

// ----------------------------------------------------------------------------- face path

/// `BM_mesh_calc_path_face`.
pub(crate) fn path_face(
    bm: &BMesh,
    src: FaceId,
    dst: FaceId,
    p: PathParams,
) -> Option<Vec<FaceId>> {
    let cap = bm.faces.ids().map(|s| s as usize + 1).max().unwrap_or(0);
    let mut tag = vec![false; cap];
    for f in bm.face_ids() {
        tag[f.slot()] = bm.face(f).flag.contains(ElemFlag::HIDDEN);
    }
    let mut prev: Vec<Option<FaceId>> = vec![None; cap];
    let mut cost = vec![f32::MAX; cap];
    let mut heap = HeapSimple::new();
    heap.insert(0.0, src);
    cost[src.slot()] = 0.0;
    // Start measuring the face path at the face edges, ignoring their centres.
    let endpoints = [src, dst];
    let mut f = src;
    while let Some(x) = heap.pop_min() {
        f = x;
        if f == dst {
            break;
        }
        if tag[f.slot()] {
            continue;
        }
        tag[f.slot()] = true;
        // `facetag_add_adjacent`.
        let f_a = f;
        let mut relax = |f_b: FaceId, cut: &dyn Fn() -> f32, heap: &mut HeapSimple<FaceId>| {
            if tag[f_b.slot()] {
                return;
            }
            let cut = if p.use_topology_distance { 1.0 } else { cut() };
            let new = cost[f_a.slot()] + cut;
            if cost[f_b.slot()] > new {
                cost[f_b.slot()] = new;
                prev[f_b.slot()] = Some(f_a);
                heap.insert(new, f_b);
            }
        };
        let loops = bm.face_loops(f_a);
        for &l_a in &loops {
            let mut it = l_a;
            loop {
                let f_b = bm.lp(it).f;
                let e = bm.lp(it).e;
                // `facetag_cut_cost_edge`.
                let cut = || {
                    let a = face_center_median_weighted(bm, f_a);
                    let b = face_center_median_weighted(bm, f_b);
                    let [v1, v2] = bm.edge(e).v;
                    let (c1, c2) = (co(bm, v1), co(bm, v2));
                    // For triangle fans it gives better results to pick a point on the edge.
                    let e_cent = match isect_line_line_v3(c1, c2, a, b) {
                        Some(ix) => {
                            let factor = line_point_factor_v3(ix, c1, c2);
                            if factor < 0.0 {
                                c1
                            } else if factor > 1.0 {
                                c2
                            } else {
                                ix
                            }
                        }
                        // Blender reads the uninitialised `ix_e` here; frap uses the
                        // edge's first vertex.
                        None => c1,
                    };
                    step_cost_3_v3_ex(a, e_cent, b, f_a == endpoints[0], f_b == endpoints[1])
                };
                relax(f_b, &cut, &mut heap);
                it = bm.lp(it).radial_next;
                if it == l_a {
                    break;
                }
            }
        }
        if p.use_step_face {
            for &l_a in &loops {
                let v = bm.lp(l_a).v;
                for l_b in bm.vert_loops(v) {
                    if l_a == l_b || loop_share_edge_check(bm, l_a, l_b) {
                        continue;
                    }
                    let f_b = bm.lp(l_b).f;
                    // `facetag_cut_cost_vert`.
                    let cut = || {
                        let a = face_center_median_weighted(bm, f_a);
                        let b = face_center_median_weighted(bm, f_b);
                        step_cost_3_v3_ex(a, co(bm, v), b, f_a == endpoints[0], f_b == endpoints[1])
                    };
                    relax(f_b, &cut, &mut heap);
                }
            }
        }
    }
    (f == dst).then(|| unwind(dst, FaceId::slot, &prev))
}

/// `BM_loop_share_edge_check`.
fn loop_share_edge_check(bm: &BMesh, a: LoopId, b: LoopId) -> bool {
    let (la, lb) = (bm.lp(a), bm.lp(b));
    let a_prev_e = bm.lp(la.prev).e;
    let b_prev_e = bm.lp(lb.prev).e;
    la.e == lb.e || la.e == b_prev_e || lb.e == la.e || lb.e == a_prev_e
}

// --------------------------------------------------------------------------- path region

/// The elements of a path region (unordered).
pub(crate) enum Region {
    Verts(Vec<VertId>),
    Edges(Vec<EdgeId>),
    Faces(Vec<FaceId>),
}

/// An endpoint of a path region.
#[derive(Copy, Clone, Debug)]
pub(crate) enum Elem {
    Vert(VertId),
    Edge(EdgeId),
    Face(FaceId),
}

/// `BM_vert_is_edge_pair_manifold`.
fn vert_is_edge_pair_manifold(bm: &BMesh, v: VertId) -> bool {
    let Some(e) = bm.vert(v).e else {
        return false;
    };
    let e_other = bm.disk_edge_next(e, v);
    e_other != e
        && bm.disk_edge_next(e_other, v) == e
        && bm.edge_is_manifold(e)
        && bm.edge_is_manifold(e_other)
}

/// `bm_vert_pair_ends`: the vertices at both ends of the edge chain through `v_pivot`
/// (`None` when the chain loops back).
fn vert_pair_ends(bm: &BMesh, v_pivot: VertId) -> Option<[VertId; 2]> {
    let first = bm.vert(v_pivot).e?;
    let mut ends = [v_pivot; 2];
    let mut j = 0;
    let mut e = first;
    loop {
        let mut e_chain = e;
        let mut v_other = bm.edge_other_vert(e_chain, v_pivot);
        while vert_is_edge_pair_manifold(bm, v_other) {
            let e_next = bm.disk_edge_next(e_chain, v_other);
            v_other = bm.edge_other_vert(e_next, v_other);
            if v_other == v_pivot {
                return None;
            }
            e_chain = e_next;
        }
        if j < 2 {
            ends[j] = v_other;
        }
        j += 1;
        e = bm.disk_edge_next(e, v_pivot);
        if e == first {
            break;
        }
    }
    Some(ends)
}

/// `mesh_calc_path_region_elem`, with `vtag` the walk-blocking vertex tags.
pub(crate) fn path_region(bm: &BMesh, src: Elem, dst: Elem) -> Option<Region> {
    let vcap = bm.verts.ids().map(|s| s as usize + 1).max().unwrap_or(0);
    // `BM_ELEM_TAG` of the vertices: blocked vertices, from the filter of each element type.
    let mut vtag = vec![false; vcap];
    let mut etag: Vec<bool> = Vec::new();
    let mut ftag: Vec<bool> = Vec::new();
    match src {
        Elem::Vert(_) => {
            for v in bm.vert_ids() {
                vtag[v.slot()] = bm.vert(v).flag.contains(ElemFlag::HIDDEN);
            }
        }
        Elem::Edge(_) => {
            vtag.iter_mut().for_each(|t| *t = true);
            etag = vec![false; bm.edges.ids().map(|s| s as usize + 1).max().unwrap_or(0)];
            for e in bm.edge_ids() {
                let hidden = bm.edge(e).flag.contains(ElemFlag::HIDDEN);
                etag[e.slot()] = hidden;
                if !hidden {
                    for v in bm.edge(e).v {
                        vtag[v.slot()] = false;
                    }
                }
            }
        }
        Elem::Face(_) => {
            vtag.iter_mut().for_each(|t| *t = true);
            ftag = vec![false; bm.faces.ids().map(|s| s as usize + 1).max().unwrap_or(0)];
            for f in bm.face_ids() {
                let hidden = bm.face(f).flag.contains(ElemFlag::HIDDEN);
                ftag[f.slot()] = hidden;
                if !hidden {
                    for v in bm.face_verts(f) {
                        vtag[v.slot()] = false;
                    }
                }
            }
        }
    }
    let verts_of = |el: Elem| -> Vec<VertId> {
        match el {
            Elem::Face(f) => bm.face_verts(f),
            Elem::Edge(e) => bm.edge(e).v.to_vec(),
            Elem::Vert(v) => vec![v],
        }
    };
    let ele_verts = [verts_of(src), verts_of(dst)];
    let use_chain_ends = !matches!(src, Elem::Face(_));
    let mut depths: [Vec<i32>; 2] = [Vec::new(), Vec::new()];
    let mut pass = 0i32;
    let mut found_all = false;
    for side in 0..2 {
        let other = 1 - side;
        depths[side] = vec![-1; vcap];
        let mut stack: Vec<VertId> = Vec::new();
        let mut stack_other: Vec<VertId> = Vec::new();
        for &v in &ele_verts[side] {
            depths[side][v.slot()] = 0;
            if bm.vert(v).e.is_some() && !vtag[v.slot()] {
                stack.push(v);
            }
        }
        // Expand the initial state to the end-points of 2-edge chains.
        if use_chain_ends {
            for &v in &ele_verts[side] {
                if vert_is_edge_pair_manifold(bm, v)
                    && let Some(ends) = vert_pair_ends(bm, v)
                {
                    for end in ends {
                        if depths[side][end.slot()] == -1 {
                            depths[side][end.slot()] = 0;
                            if !vtag[end.slot()] {
                                stack.push(end);
                            }
                        }
                    }
                }
            }
        }
        found_all = false;
        pass = 1;
        while !stack.is_empty() {
            while let Some(v_a) = stack.pop() {
                for e in bm.vert_edges(v_a) {
                    let mut v_b = bm.edge_other_vert(e, v_a);
                    if depths[side][v_b.slot()] != -1 {
                        continue;
                    }
                    // Walk along the chain until a vertex with 3+ edges.
                    let mut e_chain = e;
                    while vert_is_edge_pair_manifold(bm, v_b) && depths[side][v_b.slot()] == -1 {
                        depths[side][v_b.slot()] = pass;
                        let e_next = bm.disk_edge_next(e_chain, v_b);
                        v_b = bm.edge_other_vert(e_next, v_b);
                        e_chain = e_next;
                    }
                    if depths[side][v_b.slot()] == -1 {
                        depths[side][v_b.slot()] = pass;
                        if !vtag[v_b.slot()] {
                            stack_other.push(v_b);
                        }
                    }
                }
            }
            found_all = ele_verts[other]
                .iter()
                .all(|v| depths[side][v.slot()] != -1);
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
        return None;
    }
    let test = |v: VertId| {
        let (a, b) = (depths[0][v.slot()], depths[1][v.slot()]);
        a != -1 && b != -1 && a + b < pass
    };
    let test_chain = |v: VertId| {
        if test(v) {
            return true;
        }
        vert_is_edge_pair_manifold(bm, v)
            && vert_pair_ends(bm, v).is_some_and(|[a, b]| test(a) && test(b))
    };
    Some(match src {
        Elem::Face(_) => Region::Faces(
            bm.face_ids()
                .into_iter()
                .filter(|&f| {
                    if ftag[f.slot()] {
                        return false;
                    }
                    // Allowing a single failure on a face gives fewer gaps.
                    let mut ok_tests = if bm.face(f).len > 3 { 1 } else { 0 };
                    for v in bm.face_verts(f) {
                        if !test_chain(v) {
                            if ok_tests == 0 {
                                return false;
                            }
                            ok_tests -= 1;
                        }
                    }
                    true
                })
                .collect(),
        ),
        Elem::Edge(_) => Region::Edges(
            bm.edge_ids()
                .into_iter()
                .filter(|&e| !etag[e.slot()] && bm.edge(e).v.iter().all(|&v| test_chain(v)))
                .collect(),
        ),
        Elem::Vert(_) => Region::Verts(
            bm.vert_ids()
                .into_iter()
                .filter(|&v| test_chain(v))
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_pops_in_order_and_keeps_insertion_ties_like_blender() {
        let mut h = HeapSimple::new();
        for (v, id) in [(3.0, 'a'), (1.0, 'b'), (2.0, 'c'), (1.0, 'd'), (0.5, 'e')] {
            h.insert(v, id);
        }
        let mut out = Vec::new();
        while let Some(x) = h.pop_min() {
            out.push(x);
        }
        // Equal values pop in heap order, not insertion order: removing 'e' moves the last
        // node ('d') to the root, where it stays because 'b' is not smaller.
        assert_eq!(out, vec!['e', 'd', 'b', 'c', 'a']);
        assert_eq!(h.pop_min(), None);
    }

    #[test]
    fn step_cost_prefers_straight_paths() {
        let straight = step_cost_3_v3_ex([0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], false, false);
        let turn = step_cost_3_v3_ex([0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], false, false);
        assert_eq!(straight, 2.0 * 1.5);
        assert_eq!(turn, 2.0 * 2.0);
    }
}
