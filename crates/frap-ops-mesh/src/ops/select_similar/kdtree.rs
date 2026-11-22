// SPDX-License-Identifier: GPL-3.0-or-later
//! The `KDTree<float>`, `KDTree<float3>` and `KDTree<float4>` of `mesh.select_similar`, and
//! `ED_select_similar_compare_float(_tree)`.
//!
//! Ported from blender/source/blender/blenlib/BLI_kdtree.hh (`kdtree_insert`,
//! `kdtree_deduplicate`, `kdtree_balance`, `kdtree_find_nearest_cb`) and
//! blender/source/blender/editors/util/select_utils.cc (`ED_select_similar_compare_float`,
//! `ED_select_similar_compare_float_tree`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! The nearest search keeps Blender's single-precision squared distances
//! (`math::distance_squared`, summed left to right): a `LESS` comparison queries `FLT_MAX`,
//! whose squared distance to every node is infinite and never beats the initial `FLT_MAX`
//! bound, so Blender's `LESS` finds no node and selects nothing (reproduced).
//! `kdtree_deduplicate` sorts the nodes with `qsort` and keeps one node per distinct
//! coordinate; the coordinates that survive are the sorted distinct values whatever the sort
//! order, so a stable sort gives the same tree.

use std::cmp::Ordering;

/// `kd_node_unset`.
const UNSET: u32 = u32::MAX;

#[derive(Copy, Clone, Debug)]
struct Node<const N: usize> {
    left: u32,
    right: u32,
    co: [f32; N],
    d: usize,
}

/// `KDTree<CoordT>` with `N` dimensions.
#[derive(Clone, Debug)]
pub(crate) struct KdTree<const N: usize> {
    nodes: Vec<Node<N>>,
    root: u32,
}

/// `KDTreeNearest`: the nearest coordinate and its distance.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Nearest<const N: usize> {
    /// `co`.
    pub(crate) co: [f32; N],
    /// `dist` (`sqrtf` of the squared distance).
    pub(crate) dist: f32,
}

/// `eSimilarCmp`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SimilarCmp {
    /// `SIM_CMP_EQ`
    Eq,
    /// `SIM_CMP_GT`
    Gt,
    /// `SIM_CMP_LT`
    Lt,
}

/// `detail::distance_squared`: `math::distance_squared`, summed left to right.
fn distance_squared<const N: usize>(a: &[f32; N], b: &[f32; N]) -> f32 {
    let mut r = 0.0f32;
    for i in 0..N {
        let d = a[i] - b[i];
        if i == 0 {
            r = d * d;
        } else {
            r += d * d;
        }
    }
    r
}

/// The order `kdtree_node_cmp_deduplicate` sorts coordinates in.
fn cmp_co<const N: usize>(a: &[f32; N], b: &[f32; N]) -> Ordering {
    for j in 0..N {
        if a[j] < b[j] {
            return Ordering::Less;
        }
        if a[j] > b[j] {
            return Ordering::Greater;
        }
    }
    Ordering::Equal
}

impl<const N: usize> KdTree<N> {
    /// `kdtree_new`.
    pub(crate) fn new() -> Self {
        KdTree {
            nodes: Vec::new(),
            root: UNSET,
        }
    }

    /// `kdtree_insert`.
    pub(crate) fn insert(&mut self, co: [f32; N]) {
        self.nodes.push(Node {
            left: UNSET,
            right: UNSET,
            co,
            d: 0,
        });
    }

    /// `kdtree_deduplicate`: one node per distinct coordinate, sorted.
    pub(crate) fn deduplicate(&mut self) {
        self.nodes.sort_by(|a, b| cmp_co(&a.co, &b.co));
        self.nodes
            .dedup_by(|b, a| cmp_co(&a.co, &b.co) == Ordering::Equal);
    }

    /// `kdtree_balance`.
    pub(crate) fn balance(&mut self) {
        for n in &mut self.nodes {
            n.left = UNSET;
            n.right = UNSET;
        }
        let len = self.nodes.len();
        self.root = balance_rec(&mut self.nodes, 0, len, 0);
    }

    /// `kdtree_find_nearest`.
    pub(crate) fn find_nearest(&self, co: &[f32; N]) -> Option<Nearest<N>> {
        if self.root == UNSET {
            return None;
        }
        let mut min_dist = f32::MAX;
        let mut min_node: Option<u32> = None;
        let mut stack = vec![self.root];
        while let Some(i) = stack.pop() {
            let node = self.nodes[i as usize];
            let mut cur = node.co[node.d] - co[node.d];
            let mut test = |min_dist: &mut f32| {
                let dist_sq = distance_squared(&node.co, co);
                // `node_test_nearest` rejects only `dist_sq >= min_dist` (NaN accepted).
                #[allow(clippy::neg_cmp_op_on_partial_ord)]
                if !(dist_sq >= *min_dist) {
                    *min_dist = dist_sq;
                    min_node = Some(i);
                }
            };
            if cur < 0.0 {
                cur = -cur * cur;
                if -cur < min_dist {
                    test(&mut min_dist);
                    if node.left != UNSET {
                        stack.push(node.left);
                    }
                }
                if node.right != UNSET {
                    stack.push(node.right);
                }
            } else {
                cur *= cur;
                if cur < min_dist {
                    test(&mut min_dist);
                    if node.right != UNSET {
                        stack.push(node.right);
                    }
                }
                if node.left != UNSET {
                    stack.push(node.left);
                }
            }
        }
        min_node.map(|i| Nearest {
            co: self.nodes[i as usize].co,
            dist: min_dist.sqrt(),
        })
    }
}

impl KdTree<1> {
    /// `ED_select_similar_compare_float_tree`.
    pub(crate) fn compare(&self, length: f32, thresh: f32, compare: SimilarCmp) -> bool {
        let query = match compare {
            SimilarCmp::Eq => length,
            SimilarCmp::Gt => -1.0,
            SimilarCmp::Lt => f32::MAX,
        };
        match self.find_nearest(&[query]) {
            Some(n) => compare_float(length - n.co[0], thresh, compare),
            None => false,
        }
    }
}

/// `ED_select_similar_compare_float`.
pub(crate) fn compare_float(delta: f32, thresh: f32, compare: SimilarCmp) -> bool {
    match compare {
        SimilarCmp::Eq => delta.abs() <= thresh,
        SimilarCmp::Gt => (delta + thresh) >= 0.0,
        SimilarCmp::Lt => (delta - thresh) <= 0.0,
    }
}

/// `detail::kdtree_balance` over `all[ofs..ofs + len]`.
fn balance_rec<const N: usize>(all: &mut [Node<N>], ofs: usize, len: usize, axis: usize) -> u32 {
    if len == 0 {
        return UNSET;
    }
    if len == 1 {
        return ofs as u32;
    }
    let nodes = &mut all[ofs..ofs + len];
    let mut left: i64 = 0;
    let mut right: i64 = len as i64 - 1;
    let median = (len / 2) as i64;
    while right > left {
        let co = nodes[right as usize].co[axis];
        let mut i = left - 1;
        let mut j = right;
        loop {
            loop {
                i += 1;
                if nodes[i as usize].co[axis] >= co {
                    break;
                }
            }
            loop {
                j -= 1;
                if !(nodes[j as usize].co[axis] > co && j > left) {
                    break;
                }
            }
            if i >= j {
                break;
            }
            swap_head(nodes, i as usize, j as usize);
        }
        swap_head(nodes, i as usize, right as usize);
        if i >= median {
            right = i - 1;
        }
        if i <= median {
            left = i + 1;
        }
    }
    let m = median as usize;
    let next = (axis + 1) % N;
    let l = balance_rec(all, ofs, m, next);
    let r = balance_rec(all, ofs + m + 1, len - (m + 1), next);
    let node = &mut all[ofs + m];
    node.d = axis;
    node.left = l;
    node.right = r;
    (ofs + m) as u32
}

/// `SWAP(KDTreeNode_head, …)`: swaps everything but the child links.
fn swap_head<const N: usize>(nodes: &mut [Node<N>], a: usize, b: usize) {
    let (ca, da) = (nodes[a].co, nodes[a].d);
    nodes[a].co = nodes[b].co;
    nodes[a].d = nodes[b].d;
    nodes[b].co = ca;
    nodes[b].d = da;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree1(v: &[f32]) -> KdTree<1> {
        let mut t = KdTree::new();
        for &x in v {
            t.insert([x]);
        }
        t.deduplicate();
        t.balance();
        t
    }

    #[test]
    fn one_dimension_compares_like_blender() {
        let t = tree1(&[3.0, 1.0, 2.0, 2.0, 8.0]);
        assert_eq!(t.nodes.len(), 4);
        assert_eq!(t.find_nearest(&[2.4]).unwrap().co, [2.0]);
        assert!(t.compare(2.0, 0.0, SimilarCmp::Eq));
        assert!(!t.compare(2.5, 0.1, SimilarCmp::Eq));
        assert!(t.compare(1.0, 0.0, SimilarCmp::Gt));
        assert!(!t.compare(0.5, 0.1, SimilarCmp::Gt));
        // `FLT_MAX` squared is infinite: LESS never finds a node.
        assert!(!t.compare(0.0, 1.0, SimilarCmp::Lt));
        assert!(!KdTree::<1>::new().compare(1.0, 1.0, SimilarCmp::Eq));
    }

    #[test]
    fn nearest_matches_brute_force() {
        let mut t: KdTree<3> = KdTree::new();
        let mut pts = Vec::new();
        let mut s = 7u32;
        let mut rnd = || {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            ((s >> 8) % 1000) as f32 / 100.0
        };
        for _ in 0..200 {
            let p = [rnd(), rnd(), rnd()];
            pts.push(p);
            t.insert(p);
        }
        t.deduplicate();
        t.balance();
        for _ in 0..50 {
            let q = [rnd(), rnd(), rnd()];
            let best = pts
                .iter()
                .map(|p| distance_squared(p, &q))
                .fold(f32::MAX, f32::min);
            let n = t.find_nearest(&q).unwrap();
            assert_eq!(distance_squared(&n.co, &q), best);
            assert_eq!(n.dist, best.sqrt());
        }
        let mut t4: KdTree<4> = KdTree::new();
        t4.insert([0.0, 0.0, 1.0, -1.0]);
        t4.insert([0.0, 0.0, 1.0, -1.0]);
        t4.deduplicate();
        t4.balance();
        assert_eq!(t4.nodes.len(), 1);
        assert_eq!(t4.find_nearest(&[0.0, 0.0, 1.0, 0.0]).unwrap().dist, 1.0);
    }
}
