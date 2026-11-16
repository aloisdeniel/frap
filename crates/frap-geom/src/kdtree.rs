// SPDX-License-Identifier: GPL-3.0-or-later
//! 3D kd-tree, ported from `blender/source/blender/blenlib/BLI_kdtree.hh` (Blender 5.2): the
//! same quick-select balance, stack-based searches and duplicate clustering, so operations
//! built on it (`remove_doubles`, the UV sphere seam weld, glTF `merge_vertices`) pick the
//! same survivors as Blender.

use crate::normals::V3;

const UNSET: u32 = u32::MAX;

#[derive(Copy, Clone, Debug)]
struct Node {
    left: u32,
    right: u32,
    co: V3,
    index: i32,
    d: u8,
}

/// `KDTree<float3>`.
#[derive(Clone, Debug, Default)]
pub struct KdTree {
    nodes: Vec<Node>,
    root: u32,
    max_node_index: i32,
    balanced: bool,
}

#[inline]
fn dist_sq(a: V3, b: V3) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// A search result.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Nearest {
    /// Index given at insertion.
    pub index: i32,
    /// Distance (not squared).
    pub dist: f32,
    /// Coordinate.
    pub co: V3,
}

impl KdTree {
    /// `kdtree_new(capacity)`.
    pub fn new(capacity: usize) -> KdTree {
        KdTree {
            nodes: Vec::with_capacity(capacity),
            root: UNSET,
            max_node_index: -1,
            balanced: false,
        }
    }

    /// `kdtree_insert`.
    pub fn insert(&mut self, index: i32, co: V3) {
        self.nodes.push(Node {
            left: UNSET,
            right: UNSET,
            co,
            index,
            d: 0,
        });
        self.max_node_index = self.max_node_index.max(index);
        self.balanced = false;
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True without points.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // Ported from blender/source/blender/blenlib/BLI_kdtree.hh (detail::kdtree_balance).
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
                    if nodes[i as usize].co[ax] >= co {
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
        let l = Self::balance_rec(lo, axis, ofs);
        let r = Self::balance_rec(&mut hi[1..], axis, median as u32 + 1 + ofs);
        nodes[median].left = l;
        nodes[median].right = r;
        median as u32 + ofs
    }

    /// `kdtree_balance`: must be called after the last insertion and before any search.
    pub fn balance(&mut self) {
        for n in &mut self.nodes {
            n.left = UNSET;
            n.right = UNSET;
        }
        self.root = Self::balance_rec(&mut self.nodes, 0, 0);
        self.balanced = true;
    }

    /// `kdtree_find_nearest`.
    pub fn find_nearest(&self, co: V3) -> Option<Nearest> {
        assert!(
            self.balanced,
            "KdTree::balance must be called before searching"
        );
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
            if cur < 0.0 {
                cur = -cur * cur;
                if -cur < min_dist {
                    let ds = dist_sq(node.co, co);
                    if ds < min_dist {
                        min_dist = ds;
                        min_node = Some(ni as usize);
                    }
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
                    let ds = dist_sq(node.co, co);
                    if ds < min_dist {
                        min_dist = ds;
                        min_node = Some(ni as usize);
                    }
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
            index: self.nodes[i].index,
            dist: min_dist.sqrt(),
            co: self.nodes[i].co,
        })
    }

    /// `kdtree_range_search_cb`: calls `cb(index, co, dist_sq)` for every point within `range`
    /// in tree traversal order; returning `false` stops the search.
    pub fn range_search_cb(&self, co: V3, range: f32, mut cb: impl FnMut(i32, V3, f32) -> bool) {
        assert!(
            self.balanced,
            "KdTree::balance must be called before searching"
        );
        if self.root == UNSET {
            return;
        }
        let range_sq = range * range;
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
                if ds <= range_sq && !cb(node.index, node.co, ds) {
                    break;
                }
                if node.left != UNSET {
                    stack.push(node.left);
                }
                if node.right != UNSET {
                    stack.push(node.right);
                }
            }
        }
    }

    /// `kdtree_range_search`: all points within `range`, sorted by distance (unstable sort as
    /// `qsort`; equal distances keep traversal order here).
    pub fn range_search(&self, co: V3, range: f32) -> Vec<Nearest> {
        let mut out = Vec::new();
        self.range_search_cb(co, range, |index, c, ds| {
            out.push(Nearest {
                index,
                dist: ds,
                co: c,
            });
            true
        });
        out.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        for n in &mut out {
            n.dist = n.dist.sqrt();
        }
        out
    }

    // Ported from blender/source/blender/blenlib/BLI_kdtree.hh (kdtree_calc_duplicates_cb).
    /// Clusters points closer than `range`. `duplicates[i]` must be `-1` (free) or `i` (a kept
    /// target, when `has_self_index`); on return every clustered point holds its target index.
    /// `duplicates_cb(cluster)` picks the target as an index into `cluster`; the last element
    /// of `cluster` is the point the search started from. Returns the number of points merged.
    pub fn calc_duplicates_cb(
        &self,
        range: f32,
        duplicates: &mut [i32],
        has_self_index: bool,
        mut duplicates_cb: impl FnMut(&[i32]) -> usize,
    ) -> usize {
        assert!(
            self.balanced,
            "KdTree::balance must be called before searching"
        );
        if self.root == UNSET {
            return 0;
        }
        let mut found = 0usize;
        if has_self_index {
            let mut best = vec![0.0f32; (self.max_node_index + 1).max(0) as usize];
            for node in &self.nodes {
                let node_index = node.index;
                if duplicates[node_index as usize] != node_index {
                    continue;
                }
                self.range_search_cb(node.co, range, |neighbor, _, ds| {
                    let n = neighbor as usize;
                    let target = duplicates[n];
                    if target == -1 {
                        duplicates[n] = node_index;
                        best[n] = ds;
                        found += 1;
                    } else if target != neighbor
                        && (ds < best[n] || (ds == best[n] && node_index < target))
                    {
                        best[n] = ds;
                        duplicates[n] = node_index;
                    }
                    true
                });
            }
        }
        let mut cluster: Vec<i32> = Vec::new();
        for node in &self.nodes {
            let node_index = node.index;
            if duplicates[node_index as usize] != -1 {
                continue;
            }
            self.range_search_cb(node.co, range, |neighbor, _, _| {
                if duplicates[neighbor as usize] == -1 {
                    cluster.push(neighbor);
                }
                true
            });
            if cluster.is_empty() {
                continue;
            }
            found += cluster.len();
            cluster.push(node_index);
            let k = duplicates_cb(&cluster);
            let target = cluster[k];
            for &c in &cluster {
                duplicates[c as usize] = target;
            }
            cluster.clear();
        }
        found
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_removedoubles.cc
// (bmesh_find_doubles_by_distance_impl, deduplicate_target_calc_fn).
/// `remove_doubles` clustering: for each point, the index of the point it merges into
/// (itself when kept). `keep[i]` marks points that must survive (`keep_verts`). Returns `None`
/// only for an empty input: as in Blender, the range search reports the query point itself, so
/// every point lands in some cluster.
pub fn find_doubles_by_distance(
    coords: &[V3],
    dist: f32,
    keep: Option<&[bool]>,
) -> Option<Vec<i32>> {
    let mut tree = KdTree::new(coords.len());
    let mut duplicates = vec![-1i32; coords.len()];
    let mut has_self_index = false;
    for (i, &co) in coords.iter().enumerate() {
        tree.insert(i as i32, co);
        if keep.is_some_and(|k| k[i]) {
            duplicates[i] = i as i32;
            has_self_index = true;
        }
    }
    tree.balance();
    let found = tree.calc_duplicates_cb(dist, &mut duplicates, has_self_index, |cluster| {
        if cluster.len() == 2 {
            return if cluster[0] < cluster[1] { 0 } else { 1 };
        }
        let mut centroid = [0.0f32; 3];
        for &c in cluster {
            let p = coords[c as usize];
            centroid = [centroid[0] + p[0], centroid[1] + p[1], centroid[2] + p[2]];
        }
        let n = cluster.len() as f32;
        centroid = [centroid[0] / n, centroid[1] / n, centroid[2] / n];
        let end = cluster.len() - 1;
        let mut i_best = end;
        let mut best = crate::normals::len_squared_v3v3(centroid, coords[cluster[end] as usize]);
        for i in 0..end {
            let d = crate::normals::len_squared_v3v3(centroid, coords[cluster[i] as usize]);
            if d > best {
                continue;
            }
            if d == best && cluster[i] > cluster[i_best] {
                continue;
            }
            i_best = i;
            best = d;
        }
        i_best
    });
    if found == 0 { None } else { Some(duplicates) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_and_range() {
        let pts: Vec<V3> = (0..50)
            .map(|i| {
                let f = i as f32;
                [libm::sinf(f) * 3.0, libm::cosf(f * 1.3) * 2.0, f * 0.1]
            })
            .collect();
        let mut t = KdTree::new(pts.len());
        for (i, p) in pts.iter().enumerate() {
            t.insert(i as i32, *p);
        }
        t.balance();
        for probe in [[0.0, 0.0, 0.0], [1.0, -1.0, 2.0], [-2.0, 1.5, 4.9]] {
            let brute = pts
                .iter()
                .enumerate()
                .min_by(|a, b| dist_sq(*a.1, probe).total_cmp(&dist_sq(*b.1, probe)))
                .map(|(i, _)| i as i32);
            assert_eq!(t.find_nearest(probe).map(|n| n.index), brute);
            let mut r: Vec<i32> = t.range_search(probe, 1.5).iter().map(|n| n.index).collect();
            r.sort_unstable();
            let mut b: Vec<i32> = pts
                .iter()
                .enumerate()
                .filter(|(_, p)| dist_sq(**p, probe) <= 1.5 * 1.5)
                .map(|(i, _)| i as i32)
                .collect();
            b.sort_unstable();
            assert_eq!(r, b);
        }
    }

    #[test]
    fn doubles_merge_pairs_and_clusters() {
        let pts = [
            [0.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [0.0, 0.0, 1e-4],
            [5.0, 1e-4, 0.0],
            [9.0, 9.0, 9.0],
            [5.0, -1e-4, 0.0],
        ];
        let d = find_doubles_by_distance(&pts, 1e-3, None).unwrap();
        assert_eq!(d[0], d[2]);
        assert_eq!(d[1], d[3]);
        assert_eq!(d[1], d[5]);
        assert_eq!(d[4], 4);
        // Blender's range search reports the query point itself, so isolated points form
        // their own cluster and map to themselves.
        let d = find_doubles_by_distance(&[[0.0; 3], [1.0, 0.0, 0.0]], 0.1, None).unwrap();
        assert_eq!(d, vec![0, 1]);
        assert!(find_doubles_by_distance(&[], 0.1, None).is_none());
    }

    #[test]
    fn keep_verts_are_targets() {
        let pts = [[0.0, 0.0, 0.0], [0.0, 0.0, 1e-4], [0.0, 0.0, 2e-4]];
        let d = find_doubles_by_distance(&pts, 1e-3, Some(&[false, false, true])).unwrap();
        assert_eq!(d, vec![2, 2, 2]);
    }
}
