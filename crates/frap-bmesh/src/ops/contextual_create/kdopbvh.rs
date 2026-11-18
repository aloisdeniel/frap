// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_kdopbvh` (`blenlib/intern/BLI_kdopbvh.cc`, Blender 5.2.4): the k-DOP bounding volume
//! hierarchy, as far as the create/fill operators query it: building (`BLI_bvhtree_new`,
//! `_insert`, `_balance`: the implicit, level-by-level "optimal split" tree), nearest-point
//! search (`BLI_bvhtree_find_nearest`, depth first) and ray casting (`BLI_bvhtree_ray_cast_ex`,
//! radius 0).
//!
//! The traversal order is Blender's, so when several primitives are exactly as close (a point
//! nearest to a vertex shared by several faces, a ray through a shared vertex) the same one
//! wins as in Blender.

#![allow(clippy::needless_range_loop)] // index loops over the k-DOP axes, as ported

use super::bli::V3;

/// `bvhtree_kdop_axes`.
const KDOP_AXES: [V3; 13] = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, -1.0, 0.0],
    [1.0, 0.0, -1.0],
    [0.0, 1.0, -1.0],
];

/// `bvhtree_kdop_axes_length`.
const KDOP_AXES_LENGTH: [f32; 13] = [
    1.0,
    1.0,
    1.0,
    1.732_050_8,
    1.732_050_8,
    1.732_050_8,
    1.732_050_8,
    std::f32::consts::SQRT_2,
    std::f32::consts::SQRT_2,
    std::f32::consts::SQRT_2,
    std::f32::consts::SQRT_2,
    std::f32::consts::SQRT_2,
    std::f32::consts::SQRT_2,
];

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `std::min` / `std::max` as C++ defines them.
fn cmin(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}
fn cmax(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

#[derive(Clone, Debug)]
struct Node {
    bv: [f32; 26],
    index: i32,
    children: Vec<usize>,
    main_axis: usize,
}

impl Node {
    fn new() -> Self {
        Node {
            bv: [0.0; 26],
            index: 0,
            children: Vec::new(),
            main_axis: 0,
        }
    }
}

/// A balanced k-DOP tree (`BVHTree`).
#[derive(Clone, Debug)]
pub struct BvhTree {
    tree_type: usize,
    start_axis: usize,
    stop_axis: usize,
    epsilon: f32,
    /// `nodearray`: leaves first (insertion order), then the branches.
    nodes: Vec<Node>,
    /// `tree->nodes[0..leaf_num]`: the leaves, partitioned by the build.
    leafs: Vec<usize>,
    root: Option<usize>,
}

/// `implicit_needed_branches`.
fn implicit_needed_branches(tree_type: i32, leafs: i32) -> i32 {
    1.max((leafs + tree_type - 3) / (tree_type - 1))
}

/// `BVHBuildHelper`.
struct BuildHelper {
    leafs_num: i32,
    leafs_per_child: [i32; 32],
    branches_on_level: [i32; 32],
    remain_leafs: i32,
}

impl BuildHelper {
    /// `build_implicit_tree_helper`.
    fn new(tree_type: i32, leafs_num: i32) -> Self {
        let mut d = BuildHelper {
            leafs_num,
            leafs_per_child: [0; 32],
            branches_on_level: [0; 32],
            remain_leafs: 0,
        };
        d.leafs_per_child[0] = 1;
        while d.leafs_per_child[0] < leafs_num {
            d.leafs_per_child[0] *= tree_type;
        }
        d.branches_on_level[0] = 1;
        let mut depth = 1;
        while depth < 32 && d.leafs_per_child[depth - 1] != 0 {
            d.branches_on_level[depth] = d.branches_on_level[depth - 1] * tree_type;
            d.leafs_per_child[depth] = d.leafs_per_child[depth - 1] / tree_type;
            depth += 1;
        }
        let remain = leafs_num - d.leafs_per_child[1];
        let nnodes = (remain + tree_type - 2) / (tree_type - 1);
        d.remain_leafs = remain + nnodes;
        d
    }

    /// `implicit_leafs_index`.
    fn leafs_index(&self, depth: usize, child_index: i32) -> i32 {
        let min_leaf_index = child_index * self.leafs_per_child[depth - 1];
        if min_leaf_index <= self.remain_leafs {
            return min_leaf_index;
        }
        if self.leafs_per_child[depth] != 0 {
            return self.leafs_num
                - (self.branches_on_level[depth - 1] - child_index) * self.leafs_per_child[depth];
        }
        self.remain_leafs
    }
}

impl BvhTree {
    /// `BLI_bvhtree_new(maxsize, epsilon, tree_type, axis)` (axis 6, 8, 14, 18 or 26).
    pub fn new(epsilon: f32, tree_type: usize, axis: usize) -> Self {
        let (start_axis, stop_axis) = match axis {
            26 => (0, 13),
            18 => (7, 13),
            14 => (0, 7),
            8 => (0, 4),
            _ => (0, 3),
        };
        BvhTree {
            tree_type,
            start_axis,
            stop_axis,
            epsilon: epsilon.max(f32::EPSILON),
            nodes: Vec::new(),
            leafs: Vec::new(),
            root: None,
        }
    }

    /// `BLI_bvhtree_insert`: a leaf bounding `co` (inflated by the epsilon).
    pub fn insert(&mut self, index: i32, co: &[V3]) {
        let mut node = Node::new();
        for a in self.start_axis..self.stop_axis {
            node.bv[2 * a] = f32::MAX;
            node.bv[2 * a + 1] = -f32::MAX;
        }
        for p in co {
            for a in self.start_axis..self.stop_axis {
                let v = dot(*p, KDOP_AXES[a]);
                node.bv[2 * a] = cmin(v, node.bv[2 * a]);
                node.bv[2 * a + 1] = cmax(v, node.bv[2 * a + 1]);
            }
        }
        node.index = index;
        for a in self.start_axis..self.stop_axis {
            let d = self.epsilon * KDOP_AXES_LENGTH[a];
            node.bv[2 * a] -= d;
            node.bv[2 * a + 1] += d;
        }
        self.leafs.push(self.nodes.len());
        self.nodes.push(node);
    }

    /// `refit_kdop_hull` of node `n` over `leafs[start..end]`.
    fn refit(&mut self, n: usize, start: usize, end: usize) {
        let mut bv = [0.0f32; 26];
        for a in self.start_axis..self.stop_axis {
            bv[2 * a] = f32::MAX;
            bv[2 * a + 1] = -f32::MAX;
        }
        for j in start..end {
            let nb = &self.nodes[self.leafs[j]].bv;
            for a in self.start_axis..self.stop_axis {
                bv[2 * a] = cmin(nb[2 * a], bv[2 * a]);
                bv[2 * a + 1] = cmax(nb[2 * a + 1], bv[2 * a + 1]);
            }
        }
        self.nodes[n].bv = bv;
    }

    fn key(&self, leaf_slot: usize, axis: usize) -> f32 {
        self.nodes[self.leafs[leaf_slot]].bv[axis]
    }

    /// `bvh_insertionsort`.
    fn insertionsort(&mut self, lo: usize, hi: usize, axis: usize) {
        for i in lo..hi {
            let mut j = i;
            let t = self.leafs[i];
            let tk = self.nodes[t].bv[axis];
            while j != lo && tk < self.key(j - 1, axis) {
                self.leafs[j] = self.leafs[j - 1];
                j -= 1;
            }
            self.leafs[j] = t;
        }
    }

    /// `bvh_partition` around the value `x`.
    fn partition(&mut self, lo: usize, hi: usize, x: f32, axis: usize) -> usize {
        let mut i = lo;
        let mut j = hi;
        loop {
            while self.key(i, axis) < x {
                i += 1;
            }
            j -= 1;
            while x < self.key(j, axis) {
                j -= 1;
            }
            if i >= j {
                return i;
            }
            self.leafs.swap(i, j);
            i += 1;
        }
    }

    /// `bvh_medianof3` (the pivot's value).
    fn medianof3(&self, lo: usize, mid: usize, hi: usize, axis: usize) -> f32 {
        let (l, m, h) = (self.key(lo, axis), self.key(mid, axis), self.key(hi, axis));
        if m < l {
            if h < m {
                return m;
            }
            if h < l {
                return h;
            }
            return l;
        }
        if h < m {
            if h < l {
                return l;
            }
            return h;
        }
        m
    }

    /// `partition_nth_element`.
    fn nth_element(&mut self, mut begin: usize, mut end: usize, n: usize, axis: usize) {
        while end - begin > 3 {
            let x = self.medianof3(begin, (begin + end) / 2, end - 1, axis);
            let cut = self.partition(begin, end, x, axis);
            if cut <= n {
                begin = cut;
            } else {
                end = cut;
            }
        }
        self.insertionsort(begin, end, axis);
    }

    /// `get_largest_axis`.
    fn largest_axis(bv: &[f32; 26]) -> usize {
        let m = [bv[1] - bv[0], bv[3] - bv[2], bv[5] - bv[4]];
        if m[0] > m[1] {
            if m[0] > m[2] {
                return 1;
            }
            return 5;
        }
        if m[1] > m[2] {
            return 3;
        }
        5
    }

    /// `BLI_bvhtree_balance` (`non_recursive_bvh_div_nodes`).
    pub fn balance(&mut self) {
        let leaf_num = self.leafs.len();
        if leaf_num == 0 {
            return;
        }
        let tt = self.tree_type as i32;
        let tree_offset = 2 - tt;
        let branches_num = implicit_needed_branches(tt, leaf_num as i32);
        // Branch `j` of the implicit tree is `nodearray[leaf_num - 1 + j]`.
        let total = leaf_num + branches_num as usize + self.tree_type;
        while self.nodes.len() < total {
            self.nodes.push(Node::new());
        }
        let branch = |j: i32| leaf_num - 1 + j as usize;
        let root = branch(1);
        self.root = Some(root);
        if leaf_num == 1 {
            self.refit(root, 0, 1);
            self.nodes[root].main_axis = Self::largest_axis(&self.nodes[root].bv) / 2;
            self.nodes[root].children = vec![self.leafs[0]];
            return;
        }
        let data = BuildHelper::new(tt, leaf_num as i32);
        let mut i = 1i32;
        let mut depth = 1usize;
        while i <= branches_num {
            let first_of_next_level = i * tt + tree_offset;
            let i_stop = first_of_next_level.min(branches_num + 1);
            for j in i..i_stop {
                let parent = branch(j);
                let parent_level_index = j - i;
                let begin = data.leafs_index(depth, parent_level_index) as usize;
                let end = data.leafs_index(depth, parent_level_index + 1) as usize;
                self.refit(parent, begin, end);
                let split_axis = Self::largest_axis(&self.nodes[parent].bv);
                self.nodes[parent].main_axis = split_axis / 2;
                let mut nth = vec![0usize; self.tree_type + 1];
                nth[0] = begin;
                nth[self.tree_type] = end;
                for (k, slot) in nth.iter_mut().enumerate().take(self.tree_type).skip(1) {
                    let child_index = j * tt + tree_offset + k as i32;
                    *slot = data.leafs_index(depth + 1, child_index - first_of_next_level) as usize;
                }
                // `split_leafs`.
                for p in 0..self.tree_type - 1 {
                    if nth[p] >= nth[self.tree_type] {
                        break;
                    }
                    self.nth_element(nth[p], nth[self.tree_type], nth[p + 1], split_axis);
                }
                let mut children = Vec::new();
                for k in 0..tt {
                    let child_index = j * tt + tree_offset + k;
                    let cli = child_index - first_of_next_level;
                    let cb = data.leafs_index(depth + 1, cli);
                    let ce = data.leafs_index(depth + 1, cli + 1);
                    if ce - cb > 1 {
                        children.push(branch(child_index));
                    } else if ce - cb == 1 {
                        children.push(self.leafs[cb as usize]);
                    } else {
                        break;
                    }
                }
                self.nodes[parent].children = children;
            }
            i = i * tt + tree_offset;
            depth += 1;
        }
    }

    /// `calc_nearest_point_squared`.
    fn nearest_point_sq(proj: &[f32; 13], bv: &[f32; 26]) -> f32 {
        let mut d = 0.0f32;
        let mut nearest = [0.0f32; 3];
        for i in 0..3 {
            let mut val = proj[i];
            val = cmax(bv[2 * i], val);
            val = cmin(bv[2 * i + 1], val);
            nearest[i] = val;
        }
        for i in 0..3 {
            let t = proj[i] - nearest[i];
            d += t * t;
        }
        d
    }

    /// `BLI_bvhtree_find_nearest(co, nearest = {index -1, dist_sq}, callback)`: `callback(index,
    /// &mut dist_sq) -> bool` updates the best distance and returns whether `index` is the new
    /// best. Returns the best index.
    pub fn find_nearest(
        &self,
        co: V3,
        dist_sq: f32,
        callback: &mut dyn FnMut(i32, &mut f32) -> bool,
    ) -> Option<i32> {
        let root = self.root?;
        let mut proj = [0.0f32; 13];
        for a in self.start_axis..self.stop_axis {
            proj[a] = dot(co, KDOP_AXES[a]);
        }
        let mut best = (None, dist_sq);
        if Self::nearest_point_sq(&proj, &self.nodes[root].bv) >= best.1 {
            return None;
        }
        self.nearest_dfs(root, &proj, &mut best, callback);
        best.0
    }

    fn nearest_dfs(
        &self,
        node: usize,
        proj: &[f32; 13],
        best: &mut (Option<i32>, f32),
        callback: &mut dyn FnMut(i32, &mut f32) -> bool,
    ) {
        let n = &self.nodes[node];
        if n.children.is_empty() {
            if callback(n.index, &mut best.1) {
                best.0 = Some(n.index);
            }
            return;
        }
        let forward = proj[n.main_axis] <= self.nodes[n.children[0]].bv[n.main_axis * 2 + 1];
        let order: Vec<usize> = if forward {
            n.children.clone()
        } else {
            n.children.iter().rev().copied().collect()
        };
        for c in order {
            if Self::nearest_point_sq(proj, &self.nodes[c].bv) >= best.1 {
                continue;
            }
            self.nearest_dfs(c, proj, best, callback);
        }
    }

    /// `BLI_bvhtree_ray_cast_ex(co, dir, radius 0, hit = {index -1, dist}, callback)`:
    /// `callback(index, &mut dist) -> bool` updates the hit distance and returns whether
    /// `index` is the new hit. Returns the hit index.
    pub fn ray_cast(
        &self,
        co: V3,
        dir: V3,
        dist: f32,
        callback: &mut dyn FnMut(i32, &mut f32) -> bool,
    ) -> Option<i32> {
        let root = self.root?;
        let mut ray_dot_axis = [0.0f32; 3];
        let mut idot_axis = [0.0f32; 3];
        let mut index = [0usize; 6];
        for i in 0..3 {
            ray_dot_axis[i] = dot(dir, KDOP_AXES[i]);
            if ray_dot_axis[i].abs() < f32::EPSILON {
                ray_dot_axis[i] = 0.0;
                idot_axis[i] = f32::MAX;
            } else {
                idot_axis[i] = 1.0 / ray_dot_axis[i];
            }
            index[2 * i] = if idot_axis[i] < 0.0 { 1 } else { 0 };
            index[2 * i + 1] = 1 - index[2 * i];
            index[2 * i] += 2 * i;
            index[2 * i + 1] += 2 * i;
        }
        let rc = RayCast {
            co,
            ray_dot_axis,
            idot_axis,
            index,
        };
        let mut hit = (None, dist);
        self.raycast_dfs(root, &rc, &mut hit, callback);
        hit.0
    }

    fn raycast_dfs(
        &self,
        node: usize,
        rc: &RayCast,
        hit: &mut (Option<i32>, f32),
        callback: &mut dyn FnMut(i32, &mut f32) -> bool,
    ) {
        let n = &self.nodes[node];
        let dist = rc.fast_nearest_hit(&n.bv, hit.1);
        if dist >= hit.1 {
            return;
        }
        if n.children.is_empty() {
            if callback(n.index, &mut hit.1) {
                hit.0 = Some(n.index);
            }
            return;
        }
        if rc.ray_dot_axis[n.main_axis] > 0.0 {
            for &c in &n.children {
                self.raycast_dfs(c, rc, hit, callback);
            }
        } else {
            for &c in n.children.iter().rev() {
                self.raycast_dfs(c, rc, hit, callback);
            }
        }
    }
}

struct RayCast {
    co: V3,
    ray_dot_axis: [f32; 3],
    idot_axis: [f32; 3],
    index: [usize; 6],
}

impl RayCast {
    /// `fast_ray_nearest_hit`.
    fn fast_nearest_hit(&self, bv: &[f32; 26], hit_dist: f32) -> f32 {
        let o = self.co;
        let t1x = (bv[self.index[0]] - o[0]) * self.idot_axis[0];
        let t2x = (bv[self.index[1]] - o[0]) * self.idot_axis[0];
        let t1y = (bv[self.index[2]] - o[1]) * self.idot_axis[1];
        let t2y = (bv[self.index[3]] - o[1]) * self.idot_axis[1];
        let t1z = (bv[self.index[4]] - o[2]) * self.idot_axis[2];
        let t2z = (bv[self.index[5]] - o[2]) * self.idot_axis[2];
        if (t1x > t2y || t2x < t1y || t1x > t2z || t2x < t1z || t1y > t2z || t2y < t1z)
            || (t2x < 0.0 || t2y < 0.0 || t2z < 0.0)
            || (t1x > hit_dist || t1y > hit_dist || t1z > hit_dist)
        {
            return f32::MAX;
        }
        cmax(cmax(t1x, t1y), t1z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_and_ray_find_the_closest_leaf() {
        let mut t = BvhTree::new(1e-4, 8, 8);
        for i in 0..50 {
            let x = i as f32;
            t.insert(i, &[[x, 0.0, 0.0], [x + 0.5, 1.0, 0.0]]);
        }
        t.balance();
        let mut cb = |i: i32, d: &mut f32| {
            let dx = (i as f32 + 0.25 - 17.3).abs();
            let dsq = dx * dx;
            if dsq < *d {
                *d = dsq;
                true
            } else {
                false
            }
        };
        assert_eq!(t.find_nearest([17.3, 0.5, 0.0], f32::INFINITY, &mut cb), Some(17));
        let mut rc = |i: i32, d: &mut f32| {
            let dist = i as f32 - 3.0;
            if dist > 0.0 && dist < *d {
                *d = dist;
                true
            } else {
                false
            }
        };
        assert_eq!(t.ray_cast([3.0, 0.5, 0.0], [1.0, 0.0, 0.0], f32::MAX, &mut rc), Some(4));
    }
}
