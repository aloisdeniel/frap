// SPDX-License-Identifier: GPL-3.0-or-later
//! The `mathutils.kdtree.KDTree` calls of `uv.lightmap_pack`: `find(co, filter=…)`
//! (`kdtree_find_nearest_cb`) and `find_range(co, radius)` (`kdtree_range_search`, sorted by
//! distance with libc `qsort`). `frap-geom`'s kd-tree has neither the filtered search nor the
//! qsort order, so the tree (the same quick-select balance) is ported here.
//!
//! Ported from blender/source/blender/blenlib/BLI_kdtree.hh and
//! blender/source/blender/python/mathutils/mathutils_kdtree.cc (Blender 5.2.4).

use super::qsort::qsort;

type V3 = [f32; 3];
const UNSET: u32 = u32::MAX;

#[derive(Copy, Clone, Debug)]
struct Node {
    left: u32,
    right: u32,
    co: V3,
    index: i32,
    d: u8,
}

/// `KDTree<float3>` (balanced on construction).
#[derive(Clone, Debug)]
pub(crate) struct KdTree {
    nodes: Vec<Node>,
    root: u32,
}

fn dist_sq(a: V3, b: V3) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

impl KdTree {
    /// `KDTree(len)`, `insert(co, i)` for every point in order, then `balance()`.
    pub(crate) fn new(points: &[V3]) -> KdTree {
        let mut nodes: Vec<Node> = points
            .iter()
            .enumerate()
            .map(|(i, &co)| Node {
                left: UNSET,
                right: UNSET,
                co,
                index: i as i32,
                d: 0,
            })
            .collect();
        let root = balance(&mut nodes, 0, 0);
        KdTree { nodes, root }
    }

    /// `kdtree_find_nearest_cb` with mathutils' filter (`1`: use, `0`: skip): the index and
    /// distance (`sqrtf`) of the nearest accepted point.
    pub(crate) fn find_filtered(
        &self,
        co: V3,
        mut accept: impl FnMut(i32) -> bool,
    ) -> Option<(i32, f32)> {
        if self.root == UNSET {
            return None;
        }
        let mut min_dist = f32::MAX;
        let mut min_node: Option<usize> = None;
        let mut stack = vec![self.root];
        let mut test = |ni: usize, min_dist: &mut f32, min_node: &mut Option<usize>| {
            let node = &self.nodes[ni];
            let ds = dist_sq(node.co, co);
            if ds >= *min_dist {
                return;
            }
            if accept(node.index) {
                *min_dist = ds;
                *min_node = Some(ni);
            }
        };
        while let Some(ni) = stack.pop() {
            let node = self.nodes[ni as usize];
            let d = node.d as usize;
            let mut cur = node.co[d] - co[d];
            if cur < 0.0 {
                cur = -cur * cur;
                if -cur < min_dist {
                    test(ni as usize, &mut min_dist, &mut min_node);
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
                    test(ni as usize, &mut min_dist, &mut min_node);
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

    /// `kdtree_range_search`: the indices within `range`, sorted by distance (`qsort`).
    pub(crate) fn range(&self, co: V3, range: f32) -> Vec<i32> {
        if self.root == UNSET {
            return Vec::new();
        }
        let range_sq = range * range;
        let mut found: Vec<(i32, f32)> = Vec::new();
        let mut stack = vec![self.root];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni as usize];
            let d = node.d as usize;
            if co[d] + range < node.co[d] {
                if node.left != UNSET {
                    stack.push(node.left);
                }
            } else if co[d] - range > node.co[d] {
                if node.right != UNSET {
                    stack.push(node.right);
                }
            } else {
                let ds = dist_sq(node.co, co);
                if ds <= range_sq {
                    found.push((node.index, ds));
                }
                if node.left != UNSET {
                    stack.push(node.left);
                }
                if node.right != UNSET {
                    stack.push(node.right);
                }
            }
        }
        qsort(&mut found, &mut |a: &(i32, f32), b: &(i32, f32)| {
            if a.1 < b.1 {
                core::cmp::Ordering::Less
            } else if a.1 > b.1 {
                core::cmp::Ordering::Greater
            } else {
                core::cmp::Ordering::Equal
            }
        });
        found.into_iter().map(|(i, _)| i).collect()
    }
}

// Ported from blender/source/blender/blenlib/BLI_kdtree.hh (detail::kdtree_balance).
fn balance(nodes: &mut [Node], mut axis: u8, ofs: u32) -> u32 {
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
    axis = (axis + 1) % 3;
    let (lo, hi) = nodes.split_at_mut(median);
    let l = balance(lo, axis, ofs);
    let r = balance(&mut hi[1..], axis, median as u32 + 1 + ofs);
    nodes[median].left = l;
    nodes[median].right = r;
    median as u32 + ofs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_with_filter_and_range() {
        let pts: Vec<V3> = (0..20).map(|i| [i as f32, 0.0, 0.0]).collect();
        let t = KdTree::new(&pts);
        assert_eq!(t.find_filtered([3.2, 0.0, 0.0], |_| true).map(|r| r.0), Some(3));
        assert_eq!(
            t.find_filtered([3.2, 0.0, 0.0], |i| i != 3).map(|r| r.0),
            Some(4)
        );
        assert!(t.find_filtered([0.0; 3], |_| false).is_none());
        let r = t.range([5.0, 0.0, 0.0], 1.5);
        assert_eq!(r[0], 5);
        assert_eq!(r.len(), 3);
    }
}
