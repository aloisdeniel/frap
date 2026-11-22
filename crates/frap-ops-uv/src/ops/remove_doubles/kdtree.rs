// SPDX-License-Identifier: GPL-3.0-or-later
//! `KDTree<float2>`: the 2D kd-tree `uv.remove_doubles` merges with. Ported from
//! blender/source/blender/blenlib/BLI_kdtree.hh (`detail::kdtree_balance`,
//! `kdtree_find_nearest_cb`, `kdtree_calc_duplicates_fast`, `detail::deduplicate_recursive`),
//! Blender 5.2.4, GPL-2.0-or-later: the same quick-select balance (the node order after
//! balancing decides which UV a duplicate merges into) and stack-based search.

const UNSET: u32 = u32::MAX;

#[derive(Copy, Clone, Debug)]
struct Node {
    left: u32,
    right: u32,
    co: [f32; 2],
    index: i32,
    d: u8,
}

/// `KDTree<float2>`.
#[derive(Clone, Debug, Default)]
pub(crate) struct KdTree2 {
    nodes: Vec<Node>,
    root: u32,
}

#[inline]
fn dist_sq(a: [f32; 2], b: [f32; 2]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1]];
    d[0] * d[0] + d[1] * d[1]
}

impl KdTree2 {
    /// `kdtree_new`.
    pub(crate) fn new(capacity: usize) -> KdTree2 {
        KdTree2 {
            nodes: Vec::with_capacity(capacity),
            root: UNSET,
        }
    }

    /// `kdtree_insert`.
    pub(crate) fn insert(&mut self, index: i32, co: [f32; 2]) {
        self.nodes.push(Node {
            left: UNSET,
            right: UNSET,
            co,
            index,
            d: 0,
        });
    }

    fn balance_rec(nodes: &mut [Node], mut axis: u8, ofs: u32) -> u32 {
        let n = nodes.len();
        if n == 0 {
            return UNSET;
        }
        if n == 1 {
            return ofs;
        }
        let ax = axis as usize;
        let mut left: i64 = 0;
        let mut right: i64 = n as i64 - 1;
        let median = (n / 2) as i64;
        while right > left {
            let co = nodes[right as usize].co[ax];
            let mut i = left - 1;
            let mut j = right;
            loop {
                loop {
                    i += 1;
                    #[allow(clippy::neg_cmp_op_on_partial_ord)]
                    if !(nodes[i as usize].co[ax] < co) {
                        break;
                    }
                }
                loop {
                    j -= 1;
                    if !(nodes[j as usize].co[ax] > co && j > left) {
                        break;
                    }
                }
                if i >= j {
                    break;
                }
                nodes.swap(i as usize, j as usize);
            }
            nodes.swap(i as usize, right as usize);
            if i >= median {
                right = i - 1;
            }
            if i <= median {
                left = i + 1;
            }
        }
        let median = median as usize;
        nodes[median].d = axis;
        axis = (axis + 1) % 2;
        let (lo, hi) = nodes.split_at_mut(median);
        let l = Self::balance_rec(lo, axis, ofs);
        let r = Self::balance_rec(&mut hi[1..], axis, median as u32 + 1 + ofs);
        nodes[median].left = l;
        nodes[median].right = r;
        median as u32 + ofs
    }

    /// `kdtree_balance`.
    pub(crate) fn balance(&mut self) {
        for n in &mut self.nodes {
            n.left = UNSET;
            n.right = UNSET;
        }
        self.root = Self::balance_rec(&mut self.nodes, 0, 0);
    }

    /// `kdtree_find_nearest`: the index and distance of the nearest point.
    pub(crate) fn find_nearest(&self, co: [f32; 2]) -> Option<(i32, f32)> {
        if self.root == UNSET {
            return None;
        }
        let mut min_dist = f32::MAX;
        let mut min_node: Option<usize> = None;
        let mut stack = vec![self.root];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni as usize];
            let d = node.d as usize;
            let mut cur = node.co[d] - co[d];
            let mut test = |min_dist: &mut f32| {
                let ds = dist_sq(node.co, co);
                #[allow(clippy::neg_cmp_op_on_partial_ord)]
                if !(ds >= *min_dist) {
                    *min_dist = ds;
                    min_node = Some(ni as usize);
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
        min_node.map(|i| (self.nodes[i].index, min_dist.sqrt()))
    }

    /// `detail::deduplicate_recursive`.
    fn deduplicate(
        &self,
        i: u32,
        search: i32,
        search_co: [f32; 2],
        range: f32,
        duplicates: &mut [i32],
        found: &mut i32,
    ) {
        let node = &self.nodes[i as usize];
        let d = node.d as usize;
        if search_co[d] + range <= node.co[d] {
            if node.left != UNSET {
                self.deduplicate(node.left, search, search_co, range, duplicates, found);
            }
        } else if search_co[d] - range >= node.co[d] {
            if node.right != UNSET {
                self.deduplicate(node.right, search, search_co, range, duplicates, found);
            }
        } else {
            if search != node.index
                && duplicates[node.index as usize] == -1
                && dist_sq(node.co, search_co) <= range * range
            {
                duplicates[node.index as usize] = search;
                *found += 1;
            }
            if node.left != UNSET {
                self.deduplicate(node.left, search, search_co, range, duplicates, found);
            }
            if node.right != UNSET {
                self.deduplicate(node.right, search, search_co, range, duplicates, found);
            }
        }
    }

    /// `kdtree_calc_duplicates_fast(tree, range, use_index_order=false, duplicates)`.
    pub(crate) fn calc_duplicates_fast(&self, range: f32, duplicates: &mut [i32]) -> i32 {
        let mut found = 0;
        if self.root == UNSET {
            return 0;
        }
        for node in &self.nodes {
            let index = node.index;
            let dup = duplicates[index as usize];
            if dup == -1 || dup == index {
                let found_prev = found;
                self.deduplicate(self.root, index, node.co, range, duplicates, &mut found);
                if found != found_prev {
                    // Prevent chains of doubles.
                    duplicates[index as usize] = index;
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_and_duplicates() {
        let pts = [[0.0, 0.0], [0.5, 0.5], [0.501, 0.5], [1.0, 0.0], [0.0, 0.0001]];
        let mut t = KdTree2::new(pts.len());
        for (i, p) in pts.iter().enumerate() {
            t.insert(i as i32, *p);
        }
        t.balance();
        assert_eq!(t.find_nearest([0.9, 0.1]).map(|n| n.0), Some(3));
        let mut dups = vec![-1; pts.len()];
        let found = t.calc_duplicates_fast(0.01, &mut dups);
        assert_eq!(found, 2);
        // Each pair merges into one of its two points, which keeps itself.
        assert!(dups[1] == 1 || dups[2] == 2);
        assert!(dups[0] == 0 || dups[4] == 4);
        assert_eq!(dups[3], -1);
    }
}
