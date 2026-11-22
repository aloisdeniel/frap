// SPDX-License-Identifier: GPL-3.0-or-later
//! `find_adjacent.select_next` / `select_prev`: the next element continuing the pattern of the
//! last two selected ones, by vertex depths.
//!
//! Ported from blender/scripts/startup/bl_operators/bmesh/find_adjacent.py (Blender 5.2.4,
//! GPL-2.0-or-later). Python's sets are iterated in hash order there, but every result kept
//! is order independent (maxima of whole candidate sets, a single survivor or nothing), so
//! frap iterates in element order.

use std::collections::{BTreeMap, BTreeSet};

use frap_bmesh::{BMElem, BMesh, EdgeId, VertId};

/// `other_edges_over_face`.
fn other_edges_over_face(bm: &BMesh, e: EdgeId) -> Vec<EdgeId> {
    let mut out = Vec::new();
    for l in bm.edge_loops(e) {
        out.push(bm.lp(bm.lp(l).next).e);
        out.push(bm.lp(bm.lp(l).prev).e);
    }
    out
}

/// `other_edges_over_edge` (the wire test is on `e` itself, as in the script).
fn other_edges_over_edge(bm: &BMesh, e: EdgeId) -> Vec<EdgeId> {
    let mut out = Vec::new();
    for v in bm.edge(e).v {
        for o in bm.vert_edges(v) {
            if o != e && !bm.edge_is_wire(e) {
                out.push(o);
            }
        }
    }
    out
}

#[derive(Copy, Clone)]
enum Step {
    Edge,
    Face,
}

fn other_edges(bm: &BMesh, e: EdgeId, step: Step) -> Vec<EdgeId> {
    match step {
        Step::Edge => other_edges_over_edge(bm, e),
        Step::Face => other_edges_over_face(bm, e),
    }
}

/// `verts_from_elem`.
fn verts_from_elem(bm: &BMesh, ele: BMElem) -> Vec<VertId> {
    match ele {
        BMElem::Face(f) => bm.face_loops(f).into_iter().map(|l| bm.lp(l).v).collect(),
        BMElem::Edge(e) => bm.edge(e).v.to_vec(),
        BMElem::Vert(v) => vec![v],
    }
}

/// `edges_from_elem`.
fn edges_from_elem(bm: &BMesh, ele: BMElem) -> Vec<EdgeId> {
    match ele {
        BMElem::Face(f) => bm.face_loops(f).into_iter().map(|l| bm.lp(l).e).collect(),
        BMElem::Edge(e) => vec![e],
        BMElem::Vert(v) => bm.vert_edges(v),
    }
}

/// One breadth-first ring step over edges: calls `visit` with every element of the current
/// ring and returns the next ring.
fn next_ring(
    bm: &BMesh,
    stack_old: &[EdgeId],
    visited: &mut BTreeSet<EdgeId>,
    step: Step,
    mut visit: impl FnMut(EdgeId),
) -> Vec<EdgeId> {
    let mut stack_new = Vec::new();
    for &e in stack_old {
        visit(e);
        for o in other_edges(bm, e, step) {
            if visited.insert(o) {
                stack_new.push(o);
            }
        }
    }
    stack_new
}

/// `elems_depth_search`: the elements of `ele_init`'s type whose vertex depths from it are a
/// permutation of `depths`.
fn elems_depth_search(
    bm: &BMesh,
    ele_init: BMElem,
    depths: &[i64],
    step: Step,
    results_init: Option<&BTreeSet<BMElem>>,
) -> BTreeSet<BMElem> {
    let depth_max = depths.iter().copied().max().unwrap_or(0);
    let depth_min = depths.iter().copied().min().unwrap_or(0);
    let mut depths_sorted = depths.to_vec();
    depths_sorted.sort_unstable();
    let mut stack_old = edges_from_elem(bm, ele_init);
    let mut visited: BTreeSet<EdgeId> = stack_old.iter().copied().collect();
    let mut vert_depths: BTreeMap<VertId, i64> = BTreeMap::new();
    let mut depth = 0i64;
    while !stack_old.is_empty() && depth <= depth_max {
        stack_old = next_ring(bm, &stack_old, &mut visited, step, |e| {
            for v in bm.edge(e).v {
                vert_depths.entry(v).or_insert(depth);
            }
        });
        depth += 1;
    }
    let mut test_ele: BTreeSet<BMElem> = BTreeSet::new();
    for (&v, &d) in &vert_depths {
        if d < depth_min {
            continue;
        }
        match ele_init {
            BMElem::Face(_) => {
                for l in bm.vert_loops(v) {
                    test_ele.insert(BMElem::Face(bm.lp(l).f));
                }
            }
            BMElem::Edge(_) => {
                for e in bm.vert_edges(v) {
                    if !bm.edge_is_wire(e) {
                        test_ele.insert(BMElem::Edge(e));
                    }
                }
            }
            BMElem::Vert(_) => {
                test_ele.insert(BMElem::Vert(v));
            }
        }
    }
    let mut result = BTreeSet::new();
    for ele in test_ele {
        let verts = verts_from_elem(bm, ele);
        if verts.len() != depths.len() {
            continue;
        }
        if results_init.is_some_and(|r| !r.contains(&ele)) {
            continue;
        }
        let test: Option<Vec<i64>> = verts.iter().map(|v| vert_depths.get(v).copied()).collect();
        if let Some(mut test) = test {
            test.sort_unstable();
            if test == depths_sorted {
                result.insert(ele);
            }
        }
    }
    result
}

/// `elems_depth_measure`: the depths of `ele_dst`'s vertices from `ele_src`, aligned with
/// them (`None` when one is unreachable).
fn elems_depth_measure(bm: &BMesh, ele_dst: BMElem, ele_src: BMElem, step: Step) -> Option<Vec<i64>> {
    let mut stack_old = edges_from_elem(bm, ele_src);
    let mut visited: BTreeSet<EdgeId> = stack_old.iter().copied().collect();
    let dst_verts = verts_from_elem(bm, ele_dst);
    let mut all_dst: BTreeSet<VertId> = dst_verts.iter().copied().collect();
    let mut vert_depths: BTreeMap<VertId, i64> = BTreeMap::new();
    let mut depth = 0i64;
    while !stack_old.is_empty() && !all_dst.is_empty() {
        stack_old = next_ring(bm, &stack_old, &mut visited, step, |e| {
            for v in bm.edge(e).v {
                if all_dst.remove(&v) {
                    vert_depths.insert(v, depth);
                }
            }
        });
        depth += 1;
    }
    if all_dst.is_empty() {
        Some(dst_verts.iter().map(|v| vert_depths[v]).collect())
    } else {
        None
    }
}

/// `find_next`.
fn find_next(bm: &BMesh, ele_dst: BMElem, ele_src: BMElem) -> Vec<BMElem> {
    let (Some(src_a), Some(src_b)) = (
        elems_depth_measure(bm, ele_dst, ele_src, Step::Edge),
        elems_depth_measure(bm, ele_dst, ele_src, Step::Face),
    ) else {
        return Vec::new();
    };
    let candidates = elems_depth_search(bm, ele_dst, &src_a, Step::Edge, None);
    let mut candidates = elems_depth_search(bm, ele_dst, &src_b, Step::Face, Some(&candidates));
    candidates.remove(&ele_src);
    candidates.remove(&ele_dst);
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut diff_best = 0i64;
    let mut best: Vec<BMElem> = Vec::new();
    for &test in &candidates {
        let (Some(a), Some(b)) = (
            elems_depth_measure(bm, ele_dst, test, Step::Edge),
            elems_depth_measure(bm, ele_dst, test, Step::Face),
        ) else {
            continue;
        };
        let diff: i64 = (0..src_a.len())
            .map(|i| (src_a[i] - a[i]).pow(2) + (src_b[i] - b[i]).pow(2))
            .sum();
        if diff > diff_best {
            diff_best = diff;
            best = vec![test];
        } else if diff == diff_best {
            best.push(test);
        }
    }
    if best.len() > 1 {
        let init = std::mem::take(&mut best);
        let mut accum_max = -1i64;
        for test in init {
            let (Some(a), Some(b)) = (
                elems_depth_measure(bm, ele_src, test, Step::Edge),
                elems_depth_measure(bm, ele_src, test, Step::Face),
            ) else {
                continue;
            };
            let accum: i64 = a.iter().sum::<i64>() + b.iter().sum::<i64>();
            if accum > accum_max {
                accum_max = accum;
                best = vec![test];
            } else if accum == accum_max {
                best.push(test);
            }
        }
    }
    best
}

/// `ele_uuid`: the sorted face sizes around an element.
fn ele_uuid(bm: &BMesh, ele: BMElem) -> Vec<usize> {
    let mut ret: Vec<usize> = match ele {
        BMElem::Face(f) => {
            let mut r = Vec::new();
            for l in bm.face_loops(f) {
                for fo in bm.edge_faces(bm.lp(l).e) {
                    if fo != f {
                        r.push(bm.face(fo).len as usize);
                    }
                }
            }
            r
        }
        BMElem::Edge(e) => bm
            .edge_loops(e)
            .into_iter()
            .map(|l| bm.face(bm.lp(l).f).len as usize)
            .collect(),
        BMElem::Vert(v) => bm
            .vert_loops(v)
            .into_iter()
            .map(|l| bm.face(bm.lp(l).f).len as usize)
            .collect(),
    };
    ret.sort_unstable();
    ret
}

/// The approximations of `ele_uuid_filter`, most specific first: the sequence, its set, the
/// sum of its set, its length.
fn uuid_key(seq: &[usize], level: usize) -> Vec<usize> {
    match level {
        0 => seq.to_vec(),
        1 => {
            let mut s = seq.to_vec();
            s.dedup();
            s
        }
        2 => {
            let mut s = seq.to_vec();
            s.dedup();
            vec![s.iter().sum()]
        }
        _ => vec![seq.len()],
    }
}

/// `ele_uuid_filter`: the closest topological match, or none.
fn ele_uuid_filter(bm: &BMesh, reference: BMElem, next: &[BMElem]) -> Vec<BMElem> {
    let uuid_cmp = ele_uuid(bm, reference);
    let mut with_uuid: Vec<(BMElem, Vec<usize>)> =
        next.iter().map(|&e| (e, ele_uuid(bm, e))).collect();
    for level in 0..4 {
        let want = uuid_key(&uuid_cmp, level);
        let test: Vec<(BMElem, Vec<usize>)> = with_uuid
            .iter()
            .filter(|(_, u)| uuid_key(u, level) == want)
            .cloned()
            .collect();
        if test.len() > 1 {
            with_uuid = test;
        } else if test.len() == 1 {
            return vec![test[0].0];
        }
    }
    Vec::new()
}

/// The outcome of `select_next`.
pub(crate) enum Next {
    /// An INFO report and no change.
    Info(&'static str),
    /// The element to select and make active.
    Select(BMElem),
}

/// `select_next`: the element continuing the last two selection-history entries.
pub(crate) fn select_next(bm: &BMesh) -> Next {
    let h = &bm.select_history;
    if h.len() < 2 {
        return Next::Info("Selection pair not found");
    }
    let (dst, src) = (h[h.len() - 1], h[h.len() - 2]);
    let mut next = find_next(bm, dst, src);
    if next.len() > 1 {
        next = ele_uuid_filter(bm, dst, &next);
    }
    if next.len() != 1 {
        return Next::Info("No single next item found");
    }
    let ele = next[0];
    if bm.elem_flag(ele).contains(frap_bmesh::ElemFlag::HIDDEN) {
        return Next::Info("Next element is hidden");
    }
    Next::Select(ele)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_keys_are_increasingly_sloppy() {
        let s = [3, 4, 4, 5];
        assert_eq!(uuid_key(&s, 0), vec![3, 4, 4, 5]);
        assert_eq!(uuid_key(&s, 1), vec![3, 4, 5]);
        assert_eq!(uuid_key(&s, 2), vec![12]);
        assert_eq!(uuid_key(&s, 3), vec![4]);
    }

    #[test]
    fn a_chain_continues() {
        let mut bm = BMesh::new();
        let vs: Vec<VertId> = (0..5)
            .map(|i| bm.vert_create([i as f32, 0.0, 0.0], None, Default::default()))
            .collect();
        for w in vs.windows(2) {
            bm.edge_create(w[0], w[1], None, Default::default());
        }
        bm.select_history = vec![BMElem::Vert(vs[1]), BMElem::Vert(vs[2])];
        match select_next(&bm) {
            Next::Select(e) => assert_eq!(e, BMElem::Vert(vs[3])),
            Next::Info(m) => panic!("{m}"),
        }
        bm.select_history.clear();
        assert!(matches!(select_next(&bm), Next::Info("Selection pair not found")));
    }
}
