// SPDX-License-Identifier: GPL-3.0-or-later
//! The one-dimensional `KDTree<float>` of the `select_similar` operators and
//! `ED_select_similar_compare_float_tree`.
//!
//! Ported from blender/source/blender/blenlib/BLI_kdtree.hh (`kdtree_insert`,
//! `kdtree_deduplicate`, `kdtree_balance`, `kdtree_find_nearest`) and
//! blender/source/blender/editors/util/select_utils.cc
//! (`ED_select_similar_compare_float`, `ED_select_similar_compare_float_tree`), Blender 5.2.4,
//! GPL-2.0-or-later. The nearest search keeps Blender's squared distances in single
//! precision: comparing against `FLT_MAX` (`LESS`) squares to infinity, which never beats the
//! initial `FLT_MAX` bound, so `LESS` finds no node and selects nothing.

/// `KD_NODE_UNSET`.
const UNSET: u32 = u32::MAX;

#[derive(Copy, Clone, Debug)]
struct Node {
    left: u32,
    right: u32,
    co: f32,
    /// The split axis (always 0); `DimsNum` (1) tags a duplicate in `kdtree_deduplicate`.
    d: u32,
}

/// `KDTree<float>`.
#[derive(Debug, Default)]
pub(crate) struct KdTree1 {
    nodes: Vec<Node>,
    root: u32,
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

impl SimilarCmp {
    /// From the `compare` enum identifier.
    pub(crate) fn of(id: &str) -> SimilarCmp {
        match id {
            "GREATER" => SimilarCmp::Gt,
            "LESS" => SimilarCmp::Lt,
            _ => SimilarCmp::Eq,
        }
    }
}

impl KdTree1 {
    /// `kdtree_new`.
    pub(crate) fn new() -> KdTree1 {
        KdTree1 {
            nodes: Vec::new(),
            root: UNSET,
        }
    }

    /// `kdtree_insert`.
    pub(crate) fn insert(&mut self, co: f32) {
        self.nodes.push(Node {
            left: UNSET,
            right: UNSET,
            co,
            d: 0,
        });
    }

    /// `kdtree_deduplicate`: keeps one node per distinct value (sorted ascending).
    pub(crate) fn deduplicate(&mut self) {
        self.nodes
            .sort_by(|a, b| a.co.partial_cmp(&b.co).unwrap_or(std::cmp::Ordering::Equal));
        self.nodes.dedup_by(|b, a| a.co == b.co);
    }

    /// `kdtree_balance`.
    pub(crate) fn balance(&mut self) {
        for n in &mut self.nodes {
            n.left = UNSET;
            n.right = UNSET;
        }
        let len = self.nodes.len() as u32;
        self.root = balance_rec(&mut self.nodes, 0, len);
    }

    /// `kdtree_find_nearest`: the value of the nearest node.
    pub(crate) fn find_nearest(&self, co: f32) -> Option<f32> {
        if self.root == UNSET {
            return None;
        }
        let mut min_dist = f32::MAX;
        let mut min_node: Option<u32> = None;
        let mut stack = vec![self.root];
        let mut test = |i: u32, min_dist: &mut f32| {
            let d = self.nodes[i as usize].co - co;
            let dist_sq = d * d;
            if dist_sq < *min_dist {
                *min_dist = dist_sq;
                min_node = Some(i);
            }
        };
        while let Some(i) = stack.pop() {
            let node = self.nodes[i as usize];
            let mut cur = node.co - co;
            if cur < 0.0 {
                cur = -cur * cur;
                if -cur < min_dist {
                    test(i, &mut min_dist);
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
                    test(i, &mut min_dist);
                    if node.right != UNSET {
                        stack.push(node.right);
                    }
                }
                if node.left != UNSET {
                    stack.push(node.left);
                }
            }
        }
        min_node.map(|i| self.nodes[i as usize].co)
    }

    /// `ED_select_similar_compare_float_tree`.
    pub(crate) fn compare(&self, length: f32, thresh: f32, compare: SimilarCmp) -> bool {
        let query = match compare {
            SimilarCmp::Eq => length,
            SimilarCmp::Gt => -1.0,
            SimilarCmp::Lt => f32::MAX,
        };
        match self.find_nearest(query) {
            Some(co) => compare_float(length - co, thresh, compare),
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

/// `detail::kdtree_balance` over `nodes[ofs..ofs + len]` (one axis).
fn balance_rec(all: &mut [Node], ofs: u32, len: u32) -> u32 {
    if len == 0 {
        return UNSET;
    }
    if len == 1 {
        return ofs;
    }
    let nodes = &mut all[ofs as usize..(ofs + len) as usize];
    let mut left: i64 = 0;
    let mut right: i64 = len as i64 - 1;
    let median = (len / 2) as i64;
    while right > left {
        let co = nodes[right as usize].co;
        let mut i = left - 1;
        let mut j = right;
        loop {
            loop {
                i += 1;
                if nodes[i as usize].co >= co {
                    break;
                }
            }
            loop {
                j -= 1;
                if !(nodes[j as usize].co > co && j > left) {
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
    let m = median as u32;
    let l = balance_rec(all, ofs, m);
    let r = balance_rec(all, ofs + m + 1, len - (m + 1));
    let node = &mut all[(ofs + m) as usize];
    node.d = 0;
    node.left = l;
    node.right = r;
    ofs + m
}

/// `SWAP(KDTreeNode_head, …)`: swaps everything but the child links.
fn swap_head(nodes: &mut [Node], a: usize, b: usize) {
    let (ca, da) = (nodes[a].co, nodes[a].d);
    nodes[a].co = nodes[b].co;
    nodes[a].d = nodes[b].d;
    nodes[b].co = ca;
    nodes[b].d = da;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(v: &[f32]) -> KdTree1 {
        let mut t = KdTree1::new();
        for &x in v {
            t.insert(x);
        }
        t.deduplicate();
        t.balance();
        t
    }

    #[test]
    fn nearest_and_compare() {
        let t = tree(&[3.0, 1.0, 2.0, 2.0, 8.0]);
        assert_eq!(t.find_nearest(2.4), Some(2.0));
        assert_eq!(t.find_nearest(-1.0), Some(1.0));
        assert!(t.compare(2.0, 0.0, SimilarCmp::Eq));
        assert!(!t.compare(2.5, 0.1, SimilarCmp::Eq));
        assert!(t.compare(1.0, 0.0, SimilarCmp::Gt));
        assert!(!t.compare(0.5, 0.1, SimilarCmp::Gt));
        // `FLT_MAX` squared is infinite: Blender's LESS finds no node.
        assert!(!t.compare(0.0, 1.0, SimilarCmp::Lt));
        assert!(!KdTree1::new().compare(1.0, 1.0, SimilarCmp::Eq));
    }
}
