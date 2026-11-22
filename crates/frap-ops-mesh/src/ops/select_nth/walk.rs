// SPDX-License-Identifier: GPL-3.0-or-later
//! The breadth-first walkers of `walker_deselect_nth` with their depths: `BMW_CONNECTED_VERTEX`
//! (vertex mask), `BMW_VERT_SHELL` and `BMW_FACE_SHELL` (edge mask), `BMW_ISLAND` (face mask),
//! all with `BMW_FLAG_NOP` (hidden elements are walked too).
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_walkers_impl.cc and
//! bmesh_walkers.cc (`BMW_current_state`, `BMW_state_add`, `BMW_current_depth`), Blender
//! 5.2.4, GPL-2.0-or-later. A state records the walker depth when it is added; processing a
//! state sets the walker depth to its depth + 1, so an element's depth (`BMW_current_depth -
//! 1`) is the depth of its state: 0 for the start element.

use std::collections::VecDeque;

use frap_bmesh::{BMesh, EdgeId, FaceId, VertId};
use rustc_hash::FxHashSet;

/// `BMW_CONNECTED_VERTEX` from `v` over vertices where `mask` holds; `(vertex, depth)` in
/// walk order.
pub(crate) fn connected_vertex(
    bm: &BMesh,
    v: VertId,
    mask: &dyn Fn(VertId) -> bool,
) -> Vec<(VertId, i32)> {
    let mut out = Vec::new();
    let mut visit: FxHashSet<VertId> = FxHashSet::default();
    let mut st: VecDeque<(VertId, i32)> = VecDeque::new();
    let mut visit_vertex = |st: &mut VecDeque<(VertId, i32)>, v: VertId, depth: i32| {
        if visit.contains(&v) || !mask(v) {
            return;
        }
        st.push_back((v, depth));
        visit.insert(v);
    };
    visit_vertex(&mut st, v, 0);
    while let Some((cur, depth)) = st.pop_front() {
        for e in bm.vert_edges(cur) {
            let v2 = bm.edge_other_vert(e, cur);
            visit_vertex(&mut st, v2, depth + 1);
        }
        out.push((cur, depth));
    }
    out
}

/// `BMW_VERT_SHELL` from the edge `e` over edges where `mask` holds.
pub(crate) fn vert_shell(
    bm: &BMesh,
    e: EdgeId,
    mask: &dyn Fn(EdgeId) -> bool,
) -> Vec<(EdgeId, i32)> {
    let mut out = Vec::new();
    let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
    let mut st: VecDeque<(EdgeId, i32)> = VecDeque::new();
    let mut visit_edge = |st: &mut VecDeque<(EdgeId, i32)>, e: EdgeId, depth: i32| {
        if visit.contains(&e) || !mask(e) {
            return;
        }
        st.push_back((e, depth));
        visit.insert(e);
    };
    visit_edge(&mut st, e, 0);
    while let Some((cur, depth)) = st.pop_front() {
        for v in bm.edge(cur).v {
            for e2 in bm.vert_edges(v) {
                visit_edge(&mut st, e2, depth + 1);
            }
        }
        out.push((cur, depth));
    }
    out
}

/// `BMW_FACE_SHELL` from the edge `e` over edges where `mask` holds.
pub(crate) fn face_shell(
    bm: &BMesh,
    e: EdgeId,
    mask: &dyn Fn(EdgeId) -> bool,
) -> Vec<(EdgeId, i32)> {
    let mut out = Vec::new();
    let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
    let mut st: VecDeque<(EdgeId, i32)> = VecDeque::new();
    let mut visit_edge = |st: &mut VecDeque<(EdgeId, i32)>, e: EdgeId, depth: i32| {
        if visit.contains(&e) || !mask(e) {
            return;
        }
        st.push_back((e, depth));
        visit.insert(e);
    };
    visit_edge(&mut st, e, 0);
    while let Some((cur, depth)) = st.pop_front() {
        for l in bm.edge_loops(cur) {
            for e2 in bm.face_edges(bm.lp(l).f) {
                if e2 != cur {
                    visit_edge(&mut st, e2, depth + 1);
                }
            }
        }
        out.push((cur, depth));
    }
    out
}

/// `BMW_ISLAND` from `f` over faces where `mask` holds (no edge mask).
pub(crate) fn island(
    bm: &BMesh,
    f: FaceId,
    mask: &dyn Fn(FaceId) -> bool,
) -> Vec<(FaceId, i32)> {
    let mut out = Vec::new();
    if !mask(f) {
        return out;
    }
    let mut visit: FxHashSet<FaceId> = FxHashSet::default();
    let mut st: VecDeque<(FaceId, i32)> = VecDeque::new();
    st.push_back((f, 0));
    visit.insert(f);
    while let Some((cur, depth)) = st.pop_front() {
        for l in bm.face_loops(cur) {
            let mut lr = bm.lp(l).radial_next;
            while lr != l {
                let fo = bm.lp(lr).f;
                lr = bm.lp(lr).radial_next;
                if !mask(fo) || fo == cur || visit.contains(&fo) {
                    continue;
                }
                st.push_back((fo, depth + 1));
                visit.insert(fo);
                break;
            }
        }
        out.push((cur, depth));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(n: usize) -> (BMesh, Vec<VertId>) {
        let mut bm = BMesh::new();
        let vs: Vec<VertId> = (0..n)
            .map(|i| bm.vert_create([i as f32, 0.0, 0.0], None, Default::default()))
            .collect();
        for w in vs.windows(2) {
            bm.edge_create(w[0], w[1], None, Default::default());
        }
        (bm, vs)
    }

    #[test]
    fn depths_grow_along_a_chain() {
        let (bm, vs) = strip(4);
        let all = |_: VertId| true;
        let got = connected_vertex(&bm, vs[1], &all);
        let depths: Vec<i32> = got.iter().map(|x| x.1).collect();
        assert_eq!(depths, vec![0, 1, 1, 2]);
        let none = |_: VertId| false;
        assert!(connected_vertex(&bm, vs[1], &none).is_empty());
        let edges = bm.edge_ids();
        let ok = |_: EdgeId| true;
        let shell: Vec<i32> = vert_shell(&bm, edges[0], &ok).iter().map(|x| x.1).collect();
        assert_eq!(shell, vec![0, 1, 2]);
        // No faces: the face shell stays on its edge.
        assert_eq!(face_shell(&bm, edges[0], &ok).len(), 1);
    }
}
