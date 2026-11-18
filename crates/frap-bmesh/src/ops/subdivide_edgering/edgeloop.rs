// SPDX-License-Identifier: GPL-3.0-or-later
//! `BMEdgeLoopStore` and the parts of its API `subdivide_edgering` uses: finding the edge loops
//! of tagged edges, building a loop from vertices, flipping it, its edges, its centre and its
//! normal.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_edgeloop.cc (Blender 5.2.4,
//! GPL-2.0-or-later). The `LinkData` list becomes a `Vec` of vertices (front = `first`).

use crate::ops::subdivide_edges::math::{self, V3};
use crate::types::ElemFlag;
use crate::{BMesh, EdgeId, VertId};

/// Normals are needed even for very small loops.
const EDGELOOP_EPS: f32 = 1e-10;

/// `BMEdgeLoopStore`.
#[derive(Clone, Debug, Default)]
pub(crate) struct EdgeLoop {
    /// The vertices in order.
    pub verts: Vec<VertId>,
    /// `BM_EDGELOOP_IS_CLOSED`.
    pub closed: bool,
    /// Centre (`BM_edgeloop_calc_center`).
    pub co: V3,
    /// Normal (`BM_edgeloop_calc_normal*`, negated by `flip`).
    pub no: V3,
}

impl EdgeLoop {
    /// `BM_edgeloop_from_verts`.
    pub(crate) fn from_verts(verts: Vec<VertId>, closed: bool) -> EdgeLoop {
        EdgeLoop {
            verts,
            closed,
            ..EdgeLoop::default()
        }
    }

    /// `BM_edgeloop_length_get`.
    pub(crate) fn len(&self) -> usize {
        self.verts.len()
    }

    /// `BM_edgeloop_flip`.
    pub(crate) fn flip(&mut self) {
        self.no = math::negate(self.no);
        self.verts.reverse();
    }

    /// `BLI_listbase_rotate_first(verts, node of v)`.
    pub(crate) fn rotate_first(&mut self, v: VertId) {
        if let Some(i) = self.verts.iter().position(|&x| x == v) {
            self.verts.rotate_left(i);
        }
    }

    /// `BM_edgeloop_edges_get`.
    pub(crate) fn edges(&self, bm: &BMesh) -> Vec<EdgeId> {
        let mut out: Vec<EdgeId> = self
            .verts
            .windows(2)
            .filter_map(|w| bm.edge_exists(w[0], w[1]))
            .collect();
        if self.closed
            && let (Some(&a), Some(&b)) = (self.verts.first(), self.verts.last())
            && let Some(e) = bm.edge_exists(a, b)
        {
            out.push(e);
        }
        out
    }

    /// `BM_edgeloop_calc_center`: the edge-length weighted centre (the loop is walked as if
    /// closed).
    pub(crate) fn calc_center(&mut self, bm: &BMesh) {
        let co = |v: VertId| bm.vert(v).co;
        let n = self.verts.len();
        self.co = [0.0; 3];
        if n < 2 {
            return;
        }
        let mut totw = 0.0f32;
        let mut v_prev = co(self.verts[n - 2]);
        let mut v_curr = co(self.verts[n - 1]);
        let mut v_next = co(self.verts[0]);
        let mut w_prev = math::len_v3v3(v_prev, v_curr);
        let mut i_next = 0usize;
        loop {
            let w_curr = math::len_v3v3(v_curr, v_next);
            let w = w_curr + w_prev;
            self.co = math::madd(self.co, v_curr, w);
            totw += w;
            w_prev = w_curr;
            i_next += 1;
            if i_next >= n {
                break;
            }
            v_prev = v_curr;
            v_curr = v_next;
            v_next = co(self.verts[i_next]);
        }
        let _ = v_prev;
        if totw != 0.0 {
            self.co = math::scale(self.co, 1.0 / totw);
        }
    }

    /// `BM_edgeloop_calc_normal` (Newell's method); `false` (and `no = (…, …, 1)`) when
    /// degenerate.
    pub(crate) fn calc_normal(&mut self, bm: &BMesh) -> bool {
        let co = |v: VertId| bm.vert(v).co;
        let mut no = [0.0f32; 3];
        let mut v_prev = co(*self.verts.last().expect("loop has vertices"));
        for &v in &self.verts {
            let v_curr = co(v);
            no[0] += (v_prev[1] - v_curr[1]) * (v_prev[2] + v_curr[2]);
            no[1] += (v_prev[2] - v_curr[2]) * (v_prev[0] + v_curr[0]);
            no[2] += (v_prev[0] - v_curr[0]) * (v_prev[1] + v_curr[1]);
            v_prev = v_curr;
        }
        self.finish_normal(no)
    }

    /// `BM_edgeloop_calc_normal_aligned`.
    pub(crate) fn calc_normal_aligned(&mut self, bm: &BMesh, no_align: V3) -> bool {
        let co = |v: VertId| bm.vert(v).co;
        let mut acc = [0.0f32; 3];
        let mut v_prev = co(*self.verts.last().expect("loop has vertices"));
        for &v in &self.verts {
            let v_curr = co(v);
            let dir = math::sub(v_curr, v_prev);
            let cross = math::cross(no_align, dir);
            let no = math::cross(dir, cross);
            acc = math::add(acc, no);
            v_prev = v_curr;
        }
        self.finish_normal(acc)
    }

    fn finish_normal(&mut self, no: V3) -> bool {
        let (n, len) = math::normalize_v3(no);
        self.no = n;
        if len < EDGELOOP_EPS {
            // "Other axis set to 0.0".
            self.no[2] = 1.0;
            return false;
        }
        true
    }
}

/// `bm_vert_other_tag`: the internal-tagged edges of `v` not leading back to `v_prev`.
fn vert_other_tag(bm: &BMesh, v: VertId, v_prev: Option<VertId>) -> (usize, Option<EdgeId>) {
    let mut count = 0;
    let mut e_next = None;
    for e in bm.vert_edges(v) {
        if bm.edge(e).flag.contains(ElemFlag::INTERNAL_TAG) {
            let v_other = bm.edge_other_vert(e, v);
            if Some(v_other) != v_prev {
                e_next = Some(e);
                count += 1;
            }
        }
    }
    (count, e_next)
}

/// `bm_loop_build`: walks the tagged edges from `v` (away from `v_prev`), adding vertices at
/// the front (`dir = 1`) or the back (`dir = -1`); `false` at a branch.
fn loop_build(bm: &mut BMesh, el: &mut EdgeLoop, v_prev: VertId, v: VertId, front: bool) -> bool {
    let it = ElemFlag::INTERNAL_TAG;
    if !bm.vert(v).flag.contains(it) {
        return true;
    }
    let v_first = v;
    let mut v_prev = Some(v_prev);
    let mut v = Some(v);
    while let Some(vc) = v {
        if front {
            el.verts.insert(0, vc);
        } else {
            el.verts.push(vc);
        }
        bm.vert_mut(vc).flag.remove(it);
        let (count, e_next) = vert_other_tag(bm, vc, v_prev);
        let v_next = match (count, e_next) {
            (1, Some(e)) => {
                let vn = bm.edge_other_vert(e, vc);
                bm.edge_mut(e).flag.remove(it);
                if vn == v_first {
                    el.closed = true;
                    None
                } else {
                    Some(vn)
                }
            }
            (0, _) => None,
            _ => return false,
        };
        v_prev = Some(vc);
        v = v_next;
    }
    true
}

/// `BM_mesh_edgeloops_find(bm, r_eloops, test_fn)`: the loops (open or closed, without
/// branches, at least two vertices) of the edges passing `test`, from the first edge of each
/// in edge iteration order.
pub(crate) fn mesh_edgeloops_find(bm: &mut BMesh, test: &dyn Fn(&BMesh, EdgeId) -> bool) -> Vec<EdgeLoop> {
    let it = ElemFlag::INTERNAL_TAG;
    for v in bm.vert_ids() {
        bm.vert_mut(v).flag.remove(it);
    }
    let mut edges = Vec::new();
    for e in bm.edge_ids() {
        if test(bm, e) {
            let [a, b] = bm.edge(e).v;
            bm.edge_mut(e).flag.insert(it);
            bm.vert_mut(a).flag.insert(it);
            bm.vert_mut(b).flag.insert(it);
            edges.push(e);
        } else {
            bm.edge_mut(e).flag.remove(it);
        }
    }
    let mut out = Vec::new();
    for &e in &edges {
        if bm.edge(e).flag.contains(it) {
            let [v1, v2] = bm.edge(e).v;
            let mut el = EdgeLoop::default();
            if loop_build(bm, &mut el, v1, v2, true) && loop_build(bm, &mut el, v2, v1, false) && el.len() > 1 {
                out.push(el);
            }
        }
    }
    for &e in &edges {
        let [a, b] = bm.edge(e).v;
        bm.edge_mut(e).flag.remove(it);
        bm.vert_mut(a).flag.remove(it);
        bm.vert_mut(b).flag.remove(it);
    }
    out
}
