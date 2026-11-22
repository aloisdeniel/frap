// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from blender/source/blender/blenlib/intern/BLI_kdopbvh.cc (Blender 5.2.4).
//! k-DOP bounding-volume hierarchy: a port of
//! `blender/source/blender/blenlib/intern/BLI_kdopbvh.cc` (Blender 5.2.4, GPL-2.0-or-later)
//! with the same tree (`tree_type` children per node, `axis` = 6/8/14/18/26 k-DOP axes,
//! `epsilon` inflation), the same implicit build (`non_recursive_bvh_div_nodes`, the
//! `partition_nth_element` intro-select on the split axis' maximum) and the same traversals, so
//! queries visit and tie-break primitives as Blender's `BLI_bvhtree_*` do.
//!
//! Float rules (ARCHITECTURE §21.3): every bound and distance is computed in `f32` in Blender's
//! order; `std::min`/`std::max` keep their exact operand order ([`fmin`], [`fmax`]), which
//! decides what a NaN bound does.
//!
//! Order of results:
//! - [`BvhTree::overlap`] returns the pairs as `BLI_bvhtree_overlap` does: Blender collects
//!   them on one stack per "thread" (one thread, or one per child of the first tree's root
//!   when that tree has more than `KDOPBVH_THREAD_LEAF_THRESHOLD` = 1024 leaves in a release
//!   build) and pops each stack, so the pairs of a thread come out in reverse traversal order;
//!   the threads are concatenated in child order. frap runs the threads one after another, so
//!   the callback sees them in that order (Blender's callbacks must be thread-safe and see
//!   them interleaved).
//! - [`BvhTree::find_nearest`], [`BvhTree::ray_cast`] and [`BvhTree::range_query`] visit the
//!   nodes depth first with Blender's child-order heuristics (`main_axis`), so equal distances
//!   resolve to the same primitive.
//!
//! Not ported: skip links (`USE_SKIP_LINKS` is off in Blender), `BLI_bvhtree_walk_dfs`,
//! `BLI_bvhtree_find_nearest_projected`, `BLI_bvhtree_intersect_plane`. A difference from the
//! C code: 18-DOP trees (`axis = 18`, k-DOP axes 7..13) allocate the 26 slots of a 26-DOP
//! node, where Blender's 18-float nodes overflow into their neighbour; the x/y/z slots that
//! the split axis and the nearest queries read stay at their initial `±FLT_MAX` (uninitialised
//! memory in Blender). Blender uses no 18-DOP tree.
//!
//! Users: bone heat visibility rays (`frap-ops-object` `support::heat`, WP 8.5); the FLOAT
//! boolean's face pairs ([`BvhTree::overlap`], WP 8.6); SHRINKWRAP, DATA_TRANSFER and
//! VERTEX_WEIGHT_PROXIMITY (nearest and ray queries, W3).

mod heap;
/// Watertight ray–triangle intersection (`isect_ray_tri_watertight_v3`).
pub mod isect;
#[cfg(test)]
mod tests;

use crate::normals::V3;
pub use isect::{IsectRayPrecalc, isect_ray_tri_watertight_v3};

/// `MAX_TREETYPE`
const MAX_TREETYPE: usize = 32;
/// `KDOPBVH_THREAD_LEAF_THRESHOLD` of a release build: trees with more leaves overlap one
/// "thread" per child of the root.
const KDOPBVH_THREAD_LEAF_THRESHOLD: usize = 1024;
/// `BVH_RAYCAST_DIST_MAX`
pub const BVH_RAYCAST_DIST_MAX: f32 = f32::MAX / 2.0;
/// A missing child.
const NONE: u32 = u32::MAX;
/// Bound slots per node (26-DOP: 13 axes × min/max).
const BV: usize = 26;

/// `bvhtree_kdop_axes`
const KDOP_AXES: [[f32; 3]; 13] = [
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

/// `bvhtree_kdop_axes_length`
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

/// `std::min(a, b)`: `(b < a) ? b : a`.
#[inline]
fn fmin(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// `std::max(a, b)`: `(a < b) ? b : a`.
#[inline]
fn fmax(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `dot_v3v3`
#[inline]
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `BVHTreeNearest`: the result of a nearest-point query.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Nearest {
    /// The primitive (`-1` in Blender when nothing is within reach).
    pub index: Option<usize>,
    /// The nearest point.
    pub co: V3,
    /// The primitive's normal at it.
    pub no: V3,
    /// Squared distance to `co`.
    pub dist_sq: f32,
}

impl Nearest {
    /// A search within `sqrt(dist_sq)` that found nothing yet (`index = -1`).
    pub fn within(dist_sq: f32) -> Nearest {
        Nearest {
            index: None,
            co: [0.0; 3],
            no: [0.0; 3],
            dist_sq,
        }
    }
}

/// `BVHTreeRayHit`: the result of a ray cast.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RayHit {
    /// The primitive hit (`-1` in Blender for no hit).
    pub index: Option<usize>,
    /// Distance along the ray.
    pub dist: f32,
    /// Hit point.
    pub co: V3,
    /// Normal at the hit point.
    pub no: V3,
}

impl RayHit {
    /// No hit yet, looking up to `dist` along the ray.
    pub fn within(dist: f32) -> RayHit {
        RayHit {
            index: None,
            dist,
            co: [0.0; 3],
            no: [0.0; 3],
        }
    }
}

/// `BVHTreeRay`: what a ray-cast callback sees.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BvhRay {
    /// Ray origin.
    pub origin: V3,
    /// Ray direction (unit length).
    pub direction: V3,
    /// Radius around the ray.
    pub radius: f32,
    /// `isect_precalc` (with `BVH_RAYCAST_WATERTIGHT`).
    pub isect_precalc: Option<IsectRayPrecalc>,
}

/// The `flag` of `BLI_bvhtree_overlap_ex`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct OverlapFlags {
    /// `BVH_OVERLAP_USE_THREADING`: one stack per child of the first tree's root when it has
    /// more than 1024 leaves (decides the order of the pairs).
    pub use_threading: bool,
    /// `BVH_OVERLAP_SELF`: each symmetric pair of a tree with itself once.
    pub self_overlap: bool,
}

/// An overlap callback: `(index_a, index_b, thread) -> keep`.
pub type OverlapCallback<'a> = &'a mut dyn FnMut(usize, usize, usize) -> bool;
/// A nearest-point callback (`BVHTree_NearestPointCallback`): `(index, co, &mut nearest)`.
pub type NearestCallback<'a> = &'a mut dyn FnMut(usize, V3, &mut Nearest);
/// A ray-cast callback (`BVHTree_RayCastCallback`): `(index, &ray, &mut hit)`.
pub type RayCastCallback<'a> = &'a mut dyn FnMut(usize, &BvhRay, &mut RayHit);

/// `BVHTree` (`BLI_bvhtree_new(maxsize, epsilon, tree_type, axis)`).
///
/// The nodes live in one arena, as Blender's `nodearray`: the leaves first (in insertion
/// order), then the branches of the implicit tree; `nodes` is Blender's `tree->nodes`, the
/// leaf slots of which `balance` permutes.
#[derive(Clone, Debug)]
pub struct BvhTree {
    max_size: usize,
    epsilon: f32,
    tree_type: u8,
    axis: u8,
    start_axis: u8,
    stop_axis: u8,
    leaf_num: usize,
    branch_num: usize,
    /// `tree->nodes`: arena indices.
    nodes: Vec<u32>,
    /// Bounds of each arena node (`bv[2 * axis]` minimum, `bv[2 * axis + 1]` maximum).
    bv: Vec<[f32; BV]>,
    /// `tree_type` children per arena node ([`NONE`] when absent).
    children: Vec<u32>,
    /// Primitive index of each arena node (leaves).
    index: Vec<usize>,
    /// `node_num` (0 for a leaf).
    node_num: Vec<u8>,
    /// `main_axis` (0, 1 or 2).
    main_axis: Vec<u8>,
    /// `tree->nodes[leaf_num]` after `balance`.
    root: Option<u32>,
}

/// `implicit_needed_branches`
fn implicit_needed_branches(tree_type: i64, leafs: i64) -> i64 {
    1.max((leafs + tree_type - 3) / (tree_type - 1))
}

/// `BVHBuildHelper`
struct BuildHelper {
    leafs_num: i64,
    leafs_per_child: [i64; 32],
    branches_on_level: [i64; 32],
    remain_leafs: i64,
}

impl BuildHelper {
    /// `build_implicit_tree_helper`
    fn new(tree_type: i64, leafs_num: i64) -> BuildHelper {
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

    /// `implicit_leafs_index`: the first leaf a branch of `depth` reaches.
    fn leafs_index(&self, depth: usize, child_index: i64) -> i64 {
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
    /// `BLI_bvhtree_new`: room for `max_size` primitives, bounds inflated by `epsilon` (at
    /// least `FLT_EPSILON`), `tree_type` children per node (2..=32), `axis` k-DOP axes (6, 8,
    /// 14, 18 or 26).
    ///
    /// # Panics
    /// On a `tree_type` or `axis` Blender rejects (`BLI_assert`, then no tree).
    pub fn new(max_size: usize, epsilon: f32, tree_type: u8, axis: u8) -> BvhTree {
        assert!(
            (2..=MAX_TREETYPE as u8).contains(&tree_type),
            "BvhTree::new: tree_type {tree_type} is not in 2..=32"
        );
        let (start_axis, stop_axis) = match axis {
            26 => (0, 13),
            18 => (7, 13),
            14 => (0, 7),
            8 => (0, 4),
            6 => (0, 3),
            _ => panic!("BvhTree::new: axis {axis} is not 6, 8, 14, 18 or 26"),
        };
        BvhTree {
            max_size,
            // `max_ff(FLT_EPSILON, epsilon)`
            epsilon: if f32::EPSILON > epsilon {
                f32::EPSILON
            } else {
                epsilon
            },
            tree_type,
            axis,
            start_axis,
            stop_axis,
            leaf_num: 0,
            branch_num: 0,
            nodes: Vec::with_capacity(max_size),
            bv: Vec::with_capacity(max_size),
            children: Vec::new(),
            index: Vec::with_capacity(max_size),
            node_num: Vec::with_capacity(max_size),
            main_axis: Vec::with_capacity(max_size),
            root: None,
        }
    }

    /// `BLI_bvhtree_get_len`: the number of primitives inserted.
    pub fn len(&self) -> usize {
        self.leaf_num
    }

    /// Whether no primitive was inserted.
    pub fn is_empty(&self) -> bool {
        self.leaf_num == 0
    }

    /// `BLI_bvhtree_get_tree_type`
    pub fn tree_type(&self) -> u8 {
        self.tree_type
    }

    /// `BLI_bvhtree_get_epsilon`
    pub fn epsilon(&self) -> f32 {
        self.epsilon
    }

    /// The `max_size` the tree was created with.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// `BLI_bvhtree_get_bounding_box`: the root's x/y/z bounds (`None` before `balance` or
    /// for an empty tree).
    pub fn bounding_box(&self) -> Option<(V3, V3)> {
        let r = self.root? as usize;
        let bv = &self.bv[r];
        Some(([bv[0], bv[2], bv[4]], [bv[1], bv[3], bv[5]]))
    }

    fn axes(&self) -> std::ops::Range<usize> {
        self.start_axis as usize..self.stop_axis as usize
    }

    fn child(&self, node: u32, k: usize) -> u32 {
        self.children[node as usize * self.tree_type as usize + k]
    }

    fn is_leaf(&self, node: u32) -> bool {
        self.node_num[node as usize] == 0
    }

    /// `node_minmax_init`
    fn minmax_init(&self, bv: &mut [f32; BV]) {
        for a in self.axes() {
            bv[2 * a] = f32::MAX;
            bv[2 * a + 1] = -f32::MAX;
        }
    }

    /// `create_kdop_hull`
    fn kdop_hull(&self, bv: &mut [f32; BV], co: &[V3], moving: bool) {
        if !moving {
            self.minmax_init(bv);
        }
        for p in co {
            for a in self.axes() {
                let v = dot(*p, KDOP_AXES[a]);
                bv[2 * a] = fmin(v, bv[2 * a]);
                bv[2 * a + 1] = fmax(v, bv[2 * a + 1]);
            }
        }
    }

    /// `bvhtree_node_inflate`
    fn inflate(&self, bv: &mut [f32; BV], dist: f32) {
        for a in self.axes() {
            let d = dist * KDOP_AXES_LENGTH[a];
            bv[2 * a] -= d;
            bv[2 * a + 1] += d;
        }
    }

    /// `BLI_bvhtree_insert`: primitive `index` bounded by the points `co`.
    ///
    /// # Panics
    /// After [`BvhTree::balance`] (Blender asserts).
    pub fn insert(&mut self, index: usize, co: &[V3]) {
        assert!(
            self.branch_num == 0 && self.root.is_none(),
            "BvhTree::insert after balance"
        );
        let mut bv = [0.0f32; BV];
        self.kdop_hull(&mut bv, co, false);
        self.inflate(&mut bv, self.epsilon);
        let node = self.leaf_num as u32;
        self.bv.push(bv);
        self.index.push(index);
        self.node_num.push(0);
        self.main_axis.push(0);
        self.nodes.push(node);
        self.leaf_num += 1;
    }

    /// `BLI_bvhtree_update_node`: new bounds for the leaf inserted `index`-th (`co_moving`
    /// extends them, a swept volume). Returns `false` past the leaves (Blender's
    /// `index > leaf_num` check).
    pub fn update_node(&mut self, index: usize, co: &[V3], co_moving: Option<&[V3]>) -> bool {
        if index > self.leaf_num || index >= self.bv.len() {
            return false;
        }
        let mut bv = self.bv[index];
        self.kdop_hull(&mut bv, co, false);
        if let Some(m) = co_moving {
            self.kdop_hull(&mut bv, m, true);
        }
        self.inflate(&mut bv, self.epsilon);
        self.bv[index] = bv;
        true
    }

    /// `BLI_bvhtree_update_tree`: refits every branch bottom-up after
    /// [`BvhTree::update_node`] (no rebalancing).
    pub fn update_tree(&mut self) {
        for slot in (self.leaf_num..self.leaf_num + self.branch_num).rev() {
            let n = self.nodes[slot];
            // `node_join`
            let mut bv = [0.0f32; BV];
            self.minmax_init(&mut bv);
            for k in 0..self.tree_type as usize {
                let c = self.child(n, k);
                if c == NONE {
                    break;
                }
                let cb = &self.bv[c as usize];
                for a in self.axes() {
                    bv[2 * a] = fmin(cb[2 * a], bv[2 * a]);
                    bv[2 * a + 1] = fmax(cb[2 * a + 1], bv[2 * a + 1]);
                }
            }
            self.bv[n as usize] = bv;
        }
    }

    /// `BLI_bvhtree_balance`: builds the tree once every primitive is inserted.
    pub fn balance(&mut self) {
        debug_assert_eq!(self.branch_num, 0, "BvhTree::balance called twice");
        if self.branch_num != 0 || self.root.is_some() {
            return;
        }
        let leafs = self.leaf_num;
        if leafs == 0 {
            // Blender builds a branch with no children, which no query enters.
            return;
        }
        let tt = self.tree_type as i64;
        let branches = implicit_needed_branches(tt, leafs as i64) as usize;
        // `numnodes = maxsize + implicit_needed_branches + tree_type`.
        let total = leafs + branches + self.tree_type as usize;
        let mut init = [0.0f32; BV];
        self.minmax_init(&mut init);
        self.bv.resize(total, init);
        self.index.resize(total, 0);
        self.node_num.resize(total, 0);
        self.main_axis.resize(total, 0);
        self.children = vec![NONE; total * self.tree_type as usize];
        self.div_nodes(leafs);
        self.branch_num = branches;
        for i in 0..branches {
            self.nodes.push((leafs + i) as u32);
        }
        self.root = Some(self.nodes[leafs]);
    }

    /// `refit_kdop_hull`: the bounds of the leaf slots `start..end` into arena node `node`.
    fn refit(&mut self, node: usize, start: usize, end: usize) {
        let mut bv = [0.0f32; BV];
        self.minmax_init(&mut bv);
        for j in start..end {
            let nb = &self.bv[self.nodes[j] as usize];
            for a in self.axes() {
                bv[2 * a] = fmin(nb[2 * a], bv[2 * a]);
                bv[2 * a + 1] = fmax(nb[2 * a + 1], bv[2 * a + 1]);
            }
        }
        self.bv[node] = bv;
    }

    /// `get_largest_axis`: the bound slot (1, 3 or 5: a maximum) of the longest x/y/z extent.
    fn largest_axis(bv: &[f32; BV]) -> usize {
        let m0 = bv[1] - bv[0];
        let m1 = bv[3] - bv[2];
        let m2 = bv[5] - bv[4];
        if m0 > m1 {
            if m0 > m2 {
                return 1;
            }
            return 5;
        }
        if m1 > m2 {
            return 3;
        }
        5
    }

    fn key(&self, slot: usize, axis: usize) -> f32 {
        self.bv[self.nodes[slot] as usize][axis]
    }

    /// `bvh_insertionsort`
    fn insertion_sort(&mut self, lo: usize, hi: usize, axis: usize) {
        for i in lo..hi {
            let mut j = i;
            let t = self.nodes[i];
            let tv = self.bv[t as usize][axis];
            while j != lo && tv < self.key(j - 1, axis) {
                self.nodes[j] = self.nodes[j - 1];
                j -= 1;
            }
            self.nodes[j] = t;
        }
    }

    /// `bvh_partition` around the value `x`.
    fn partition(&mut self, lo: usize, hi: usize, x: f32, axis: usize) -> usize {
        let (mut i, mut j) = (lo, hi);
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
            self.nodes.swap(i, j);
            i += 1;
        }
    }

    /// `bvh_medianof3`: the pivot value.
    fn median_of_3(&self, lo: usize, mid: usize, hi: usize, axis: usize) -> f32 {
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

    /// `partition_nth_element`
    fn nth_element(&mut self, mut begin: usize, mut end: usize, n: usize, axis: usize) {
        while end - begin > 3 {
            let pivot = self.median_of_3(begin, (begin + end) / 2, end - 1, axis);
            let cut = self.partition(begin, end, pivot, axis);
            if cut <= n {
                begin = cut;
            } else {
                end = cut;
            }
        }
        self.insertion_sort(begin, end, axis);
    }

    /// `split_leafs`
    fn split_leafs(&mut self, nth: &[usize], partitions: usize, axis: usize) {
        for i in 0..partitions - 1 {
            if nth[i] >= nth[partitions] {
                break;
            }
            self.nth_element(nth[i], nth[partitions], nth[i + 1], axis);
        }
    }

    /// `non_recursive_bvh_div_nodes`: branch `j` (1-based) of the implicit tree is arena node
    /// `leaf_num - 1 + j`.
    fn div_nodes(&mut self, leafs_num: usize) {
        let tt = self.tree_type as i64;
        let offset = 2 - tt;
        let branches_num = implicit_needed_branches(tt, leafs_num as i64);
        let base = leafs_num as i64 - 1;
        let root = (base + 1) as usize;
        if leafs_num == 1 {
            self.refit(root, 0, 1);
            self.main_axis[root] = (Self::largest_axis(&self.bv[root]) / 2) as u8;
            self.node_num[root] = 1;
            self.children[root * tt as usize] = self.nodes[0];
            return;
        }
        let data = BuildHelper::new(tt, leafs_num as i64);
        let mut i = 1i64;
        let mut depth = 1usize;
        while i <= branches_num {
            let first_of_next_level = i * tt + offset;
            let i_stop = first_of_next_level.min(branches_num + 1);
            for j in i..i_stop {
                self.div_task(&data, base, j, i, depth, first_of_next_level, offset);
            }
            i = first_of_next_level;
            depth += 1;
        }
    }

    /// `non_recursive_bvh_div_nodes_task_cb` for branch `j`.
    #[allow(clippy::too_many_arguments)]
    fn div_task(
        &mut self,
        data: &BuildHelper,
        base: i64,
        j: i64,
        level_start: i64,
        depth: usize,
        first_of_next_level: i64,
        offset: i64,
    ) {
        let tt = self.tree_type as i64;
        let parent = (base + j) as usize;
        let parent_level_index = j - level_start;
        let begin = data.leafs_index(depth, parent_level_index) as usize;
        let end = data.leafs_index(depth, parent_level_index + 1) as usize;
        self.refit(parent, begin, end);
        let split_axis = Self::largest_axis(&self.bv[parent]);
        self.main_axis[parent] = (split_axis / 2) as u8;
        let mut nth = [0usize; MAX_TREETYPE + 1];
        nth[0] = begin;
        nth[tt as usize] = end;
        for k in 1..tt {
            let child_index = j * tt + offset + k;
            let child_level_index = child_index - first_of_next_level;
            nth[k as usize] = data.leafs_index(depth + 1, child_level_index) as usize;
        }
        self.split_leafs(&nth, tt as usize, split_axis);
        let mut k = 0i64;
        while k < tt {
            let child_index = j * tt + offset + k;
            let child_level_index = child_index - first_of_next_level;
            let cb = data.leafs_index(depth + 1, child_level_index);
            let ce = data.leafs_index(depth + 1, child_level_index + 1);
            let slot = parent * tt as usize + k as usize;
            if ce - cb > 1 {
                self.children[slot] = (base + child_index) as u32;
            } else if ce - cb == 1 {
                self.children[slot] = self.nodes[cb as usize];
            } else {
                break;
            }
            k += 1;
        }
        self.node_num[parent] = k as u8;
    }

    // ---------------------------------------------------------------------------------------
    // Overlap
    // ---------------------------------------------------------------------------------------

    /// `BLI_bvhtree_overlap`: the pairs `(self_index, other_index)` whose bounds overlap and
    /// that `exact` accepts (`BVHTree_OverlapCallback`), in Blender's order (see the module
    /// documentation).
    pub fn overlap(
        &self,
        other: &BvhTree,
        exact: &mut dyn FnMut(usize, usize) -> bool,
    ) -> Vec<(usize, usize)> {
        let mut cb = |a: usize, b: usize, _thread: usize| exact(a, b);
        self.overlap_ex(
            other,
            Some(&mut cb),
            0,
            OverlapFlags {
                use_threading: true,
                self_overlap: false,
            },
        )
    }

    /// `BLI_bvhtree_overlap_self`: every symmetric pair of the tree with itself once (no pair
    /// of a leaf with itself).
    pub fn overlap_self(&self, exact: &mut dyn FnMut(usize, usize) -> bool) -> Vec<(usize, usize)> {
        let mut cb = |a: usize, b: usize, _thread: usize| exact(a, b);
        self.overlap_ex(
            self,
            Some(&mut cb),
            0,
            OverlapFlags {
                use_threading: true,
                self_overlap: true,
            },
        )
    }

    /// `BLI_bvhtree_overlap_ex` with `BVH_OVERLAP_RETURN_PAIRS`: `exact(a, b, thread)` filters
    /// the pairs (none: every pair of overlapping leaves); `max_interactions > 0` stops each
    /// first-tree leaf after that many accepted pairs (`tree_overlap_traverse_num`).
    pub fn overlap_ex(
        &self,
        other: &BvhTree,
        exact: Option<OverlapCallback<'_>>,
        max_interactions: u32,
        flags: OverlapFlags,
    ) -> Vec<(usize, usize)> {
        let use_threading = flags.use_threading && self.leaf_num > KDOPBVH_THREAD_LEAF_THRESHOLD;
        let same = std::ptr::eq(self, other);
        let use_self = flags.self_overlap && same;
        // Can't compare 14-DOP with 18-DOP.
        if self.axis != other.axis
            && (self.axis == 14 || other.axis == 14)
            && (self.axis == 18 || other.axis == 18)
        {
            return Vec::new();
        }
        let (Some(root1), Some(root2)) = (self.root, other.root) else {
            return Vec::new();
        };
        let start_axis = self.start_axis.min(other.start_axis) as usize;
        let stop_axis = self.stop_axis.min(other.stop_axis) as usize;
        if !overlap_test(
            &self.bv[root1 as usize],
            &other.bv[root2 as usize],
            start_axis,
            stop_axis,
        ) {
            return Vec::new();
        }
        let root_node_len = (self.tree_type as usize).min(self.node_num[root1 as usize] as usize);
        let thread_num = if use_threading { root_node_len } else { 1 };
        let mut walk = OverlapWalk {
            t1: self,
            t2: other,
            same,
            start_axis,
            stop_axis,
            callback: exact,
        };
        let mut out = Vec::new();
        for j in 0..thread_num {
            let mut th = OverlapThread {
                stack: Vec::new(),
                max_interactions: if use_self { 0 } else { max_interactions },
                thread: j,
            };
            if use_threading {
                // `bvhtree_overlap_task_cb`
                let c = self.child(root1, j);
                if use_self {
                    walk.invoke_self(&mut th, c);
                    for k in j + 1..self.node_num[root1 as usize] as usize {
                        walk.invoke(&mut th, c, self.child(root1, k));
                    }
                } else {
                    walk.invoke(&mut th, c, root2);
                }
            } else if use_self {
                walk.invoke_self(&mut th, root1);
            } else {
                walk.invoke(&mut th, root1, root2);
            }
            // `BLI_stack_pop_n`: last pushed first.
            out.extend(th.stack.into_iter().rev());
        }
        out
    }

    // ---------------------------------------------------------------------------------------
    // Nearest
    // ---------------------------------------------------------------------------------------

    /// `BLI_bvhtree_find_nearest`: the nearest primitive to `co` within `sqrt(dist_sq)`;
    /// `exact(index, co, &mut best)` refines a candidate (`BVHTree_NearestPointCallback`).
    pub fn find_nearest(
        &self,
        co: V3,
        dist_sq: f32,
        exact: &mut dyn FnMut(usize, V3, &mut Nearest),
    ) -> Option<Nearest> {
        let mut nearest = Nearest::within(dist_sq);
        self.find_nearest_ex(co, &mut nearest, Some(exact), false)?;
        Some(nearest)
    }

    /// `BLI_bvhtree_find_nearest_ex`: updates `nearest` (its `dist_sq` bounds the search) and
    /// returns its index. Without a callback a leaf's bounds stand for the primitive;
    /// `optimal_order` is `BVH_NEAREST_OPTIMAL_ORDER` (a priority queue, `BLI_heapsimple`).
    pub fn find_nearest_ex(
        &self,
        co: V3,
        nearest: &mut Nearest,
        callback: Option<NearestCallback<'_>>,
        optimal_order: bool,
    ) -> Option<usize> {
        let mut proj = [0.0f32; 13];
        for a in self.axes() {
            proj[a] = dot(co, KDOP_AXES[a]);
        }
        let mut d = NearestData {
            tree: self,
            co,
            proj,
            callback,
            nearest: *nearest,
        };
        if let Some(root) = self.root {
            if optimal_order {
                d.heap_begin(root);
            } else {
                d.dfs_begin(root);
            }
        }
        *nearest = d.nearest;
        nearest.index
    }

    // ---------------------------------------------------------------------------------------
    // Ray cast
    // ---------------------------------------------------------------------------------------

    /// `BLI_bvhtree_ray_cast`: the first hit of the ray `origin + t·dir` (`t ≤ max_dist`, a
    /// sphere of `radius` when > 0); `exact(index, origin, dir, &mut best)` tests a candidate
    /// (`BVHTree_RayCastCallback`). [`BvhTree::ray_cast_ex`] hands the callback the
    /// watertight precalculation too.
    pub fn ray_cast(
        &self,
        origin: V3,
        dir: V3,
        radius: f32,
        max_dist: f32,
        exact: &mut dyn FnMut(usize, V3, V3, &mut RayHit),
    ) -> Option<RayHit> {
        let mut hit = RayHit::within(max_dist);
        let mut cb =
            |i: usize, ray: &BvhRay, h: &mut RayHit| exact(i, ray.origin, ray.direction, h);
        self.ray_cast_ex(origin, dir, radius, &mut hit, Some(&mut cb), true)?;
        Some(hit)
    }

    /// `BLI_bvhtree_ray_cast_ex`: updates `hit` (its `dist` bounds the search) and returns its
    /// index. Without a callback a leaf's bounds stand for the primitive; `watertight` is
    /// `BVH_RAYCAST_WATERTIGHT` (fills [`BvhRay::isect_precalc`]).
    pub fn ray_cast_ex(
        &self,
        origin: V3,
        dir: V3,
        radius: f32,
        hit: &mut RayHit,
        callback: Option<RayCastCallback<'_>>,
        watertight: bool,
    ) -> Option<usize> {
        let mut d = RayCastData::new(self, origin, dir, radius, *hit, callback, watertight);
        if let Some(root) = self.root {
            d.dfs(root, false);
        }
        *hit = d.hit;
        hit.index
    }

    /// `BLI_bvhtree_ray_cast_all_ex`: calls `callback` for every primitive whose bounds the
    /// ray reaches within `hit_dist` (the distance is reset after each call).
    pub fn ray_cast_all(
        &self,
        origin: V3,
        dir: V3,
        radius: f32,
        hit_dist: f32,
        callback: RayCastCallback<'_>,
        watertight: bool,
    ) {
        let hit = RayHit::within(hit_dist);
        let mut d = RayCastData::new(self, origin, dir, radius, hit, Some(callback), watertight);
        if let Some(root) = self.root {
            d.dfs(root, true);
        }
    }

    // ---------------------------------------------------------------------------------------
    // Range query
    // ---------------------------------------------------------------------------------------

    /// `BLI_bvhtree_range_query`: every primitive whose bounds come within `radius` of `co`;
    /// `cb(index, co, dist_sq)` sees each one (`dist_sq` to its bounds). Returns the count.
    pub fn range_query(&self, co: V3, radius: f32, cb: &mut dyn FnMut(usize, V3, f32)) -> usize {
        let Some(root) = self.root else {
            return 0;
        };
        let radius_sq = radius * radius;
        let mut hits = 0;
        let dist_sq = nearest_point_squared(&co, &self.bv[root as usize]).1;
        if dist_sq < radius_sq {
            if self.is_leaf(root) {
                hits += 1;
                cb(self.index[root as usize], co, dist_sq);
            } else {
                self.dfs_range(root, co, radius_sq, &mut hits, cb);
            }
        }
        hits
    }

    /// `dfs_range_query`
    fn dfs_range(
        &self,
        node: u32,
        co: V3,
        radius_sq: f32,
        hits: &mut usize,
        cb: &mut dyn FnMut(usize, V3, f32),
    ) {
        if self.is_leaf(node) {
            return;
        }
        for i in 0..self.node_num[node as usize] as usize {
            let c = self.child(node, i);
            let dist_sq = nearest_point_squared(&co, &self.bv[c as usize]).1;
            if dist_sq < radius_sq {
                if self.is_leaf(c) {
                    *hits += 1;
                    cb(self.index[c as usize], co, dist_sq);
                } else {
                    self.dfs_range(c, co, radius_sq, hits, cb);
                }
            }
        }
    }
}

/// `tree_overlap_test`
fn overlap_test(bv1: &[f32; BV], bv2: &[f32; BV], start_axis: usize, stop_axis: usize) -> bool {
    for a in start_axis..stop_axis {
        if bv1[2 * a] > bv2[2 * a + 1] || bv2[2 * a] > bv1[2 * a + 1] {
            return false;
        }
    }
    true
}

/// `BVHOverlapData_Thread`
struct OverlapThread {
    stack: Vec<(usize, usize)>,
    max_interactions: u32,
    thread: usize,
}

/// `BVHOverlapData_Shared`
struct OverlapWalk<'a, 'cb> {
    t1: &'a BvhTree,
    t2: &'a BvhTree,
    /// `tree1 == tree2` (a leaf never overlaps itself).
    same: bool,
    start_axis: usize,
    stop_axis: usize,
    callback: Option<OverlapCallback<'cb>>,
}

impl OverlapWalk<'_, '_> {
    fn test(&self, n1: u32, n2: u32) -> bool {
        overlap_test(
            &self.t1.bv[n1 as usize],
            &self.t2.bv[n2 as usize],
            self.start_axis,
            self.stop_axis,
        )
    }

    /// `tree_overlap_invoke_traverse`
    fn invoke(&mut self, th: &mut OverlapThread, n1: u32, n2: u32) {
        if th.max_interactions != 0 {
            self.traverse_num(th, n1, n2);
        } else {
            self.traverse(th, n1, n2);
        }
    }

    /// `tree_overlap_traverse` / `tree_overlap_traverse_cb`
    fn traverse(&mut self, th: &mut OverlapThread, n1: u32, n2: u32) {
        if !self.test(n1, n2) {
            return;
        }
        if self.t1.is_leaf(n1) {
            if self.t2.is_leaf(n2) {
                if self.same && n1 == n2 {
                    return;
                }
                let (a, b) = (self.t1.index[n1 as usize], self.t2.index[n2 as usize]);
                let keep = match self.callback.as_deref_mut() {
                    Some(cb) => cb(a, b, th.thread),
                    None => true,
                };
                if keep {
                    th.stack.push((a, b));
                }
            } else {
                for j in 0..self.t2.node_num[n2 as usize] as usize {
                    self.traverse(th, n1, self.t2.child(n2, j));
                }
            }
        } else {
            for j in 0..self.t1.node_num[n1 as usize] as usize {
                self.traverse(th, self.t1.child(n1, j), n2);
            }
        }
    }

    /// `tree_overlap_traverse_num`: stops after `max_interactions` accepted pairs.
    fn traverse_num(&mut self, th: &mut OverlapThread, n1: u32, n2: u32) -> bool {
        if !self.test(n1, n2) {
            return false;
        }
        if self.t1.is_leaf(n1) {
            if self.t2.is_leaf(n2) {
                if self.same && n1 == n2 {
                    return false;
                }
                let (a, b) = (self.t1.index[n1 as usize], self.t2.index[n2 as usize]);
                let keep = match self.callback.as_deref_mut() {
                    Some(cb) => cb(a, b, th.thread),
                    None => true,
                };
                if keep {
                    th.stack.push((a, b));
                    th.max_interactions -= 1;
                    return th.max_interactions == 0;
                }
            } else {
                for j in 0..self.t2.node_num[n2 as usize] as usize {
                    if self.traverse_num(th, n1, self.t2.child(n2, j)) {
                        return true;
                    }
                }
            }
        } else {
            let max_interactions = th.max_interactions;
            for j in 0..self.t1.node_num[n1 as usize] as usize {
                if self.traverse_num(th, self.t1.child(n1, j), n2) {
                    th.max_interactions = max_interactions;
                }
            }
        }
        false
    }

    /// `tree_overlap_traverse_self` / `tree_overlap_traverse_self_cb`
    fn invoke_self(&mut self, th: &mut OverlapThread, node: u32) {
        let t = self.t1;
        for i in 0..t.node_num[node as usize] as usize {
            self.invoke_self(th, t.child(node, i));
            for j in i + 1..t.node_num[node as usize] as usize {
                self.traverse(th, t.child(node, i), t.child(node, j));
            }
        }
    }
}

/// `calc_nearest_point_squared`: the point of a node's x/y/z bounds nearest to `proj` and the
/// squared distance to it.
fn nearest_point_squared(proj: &[f32], bv: &[f32; BV]) -> (V3, f32) {
    let mut nearest = [0.0f32; 3];
    for i in 0..3 {
        let mut val = proj[i];
        val = fmax(bv[2 * i], val);
        val = fmin(bv[2 * i + 1], val);
        nearest[i] = val;
    }
    // `len_squared_v3v3(proj, nearest)`: `nearest - proj`.
    let d = [
        nearest[0] - proj[0],
        nearest[1] - proj[1],
        nearest[2] - proj[2],
    ];
    (nearest, dot(d, d))
}

/// `BVHNearestData`
struct NearestData<'a, 'cb> {
    tree: &'a BvhTree,
    co: V3,
    proj: [f32; 13],
    callback: Option<NearestCallback<'cb>>,
    nearest: Nearest,
}

impl NearestData<'_, '_> {
    fn leaf(&mut self, node: u32) {
        let idx = self.tree.index[node as usize];
        match self.callback.as_deref_mut() {
            Some(cb) => cb(idx, self.co, &mut self.nearest),
            None => {
                self.nearest.index = Some(idx);
                let (co, d) = nearest_point_squared(&self.proj, &self.tree.bv[node as usize]);
                self.nearest.co = co;
                self.nearest.dist_sq = d;
            }
        }
    }

    fn dist_sq(&self, node: u32) -> f32 {
        nearest_point_squared(&self.proj, &self.tree.bv[node as usize]).1
    }

    /// `dfs_find_nearest_dfs`
    fn dfs(&mut self, node: u32) {
        let t = self.tree;
        if t.is_leaf(node) {
            self.leaf(node);
            return;
        }
        let n = t.node_num[node as usize] as usize;
        let ma = t.main_axis[node as usize] as usize;
        let first = t.child(node, 0);
        let forward = self.proj[ma] <= t.bv[first as usize][ma * 2 + 1];
        for k in 0..n {
            let i = if forward { k } else { n - 1 - k };
            let c = t.child(node, i);
            if self.dist_sq(c) >= self.nearest.dist_sq {
                continue;
            }
            self.dfs(c);
        }
    }

    /// `dfs_find_nearest_begin`
    fn dfs_begin(&mut self, root: u32) {
        if self.dist_sq(root) >= self.nearest.dist_sq {
            return;
        }
        self.dfs(root);
    }

    /// `heap_find_nearest_inner`
    fn heap_inner(&mut self, heap: &mut heap::HeapSimple, node: u32) {
        let t = self.tree;
        if t.is_leaf(node) {
            self.leaf(node);
            return;
        }
        for i in 0..t.node_num[node as usize] as usize {
            let c = t.child(node, i);
            let d = self.dist_sq(c);
            if d < self.nearest.dist_sq {
                heap.insert(d, c);
            }
        }
    }

    /// `heap_find_nearest_begin`
    fn heap_begin(&mut self, root: u32) {
        if self.dist_sq(root) < self.nearest.dist_sq {
            let mut heap = heap::HeapSimple::with_capacity(32);
            self.heap_inner(&mut heap, root);
            while !heap.is_empty() && heap.top_value() < self.nearest.dist_sq {
                let node = heap.pop_min();
                self.heap_inner(&mut heap, node);
            }
        }
    }
}

/// `BVHRayCastData`
struct RayCastData<'a, 'cb> {
    tree: &'a BvhTree,
    callback: Option<RayCastCallback<'cb>>,
    ray: BvhRay,
    ray_dot_axis: [f32; 3],
    idot_axis: [f32; 3],
    index: [usize; 6],
    hit: RayHit,
}

impl<'a, 'cb> RayCastData<'a, 'cb> {
    /// `bvhtree_ray_cast_data_precalc`
    fn new(
        tree: &'a BvhTree,
        origin: V3,
        dir: V3,
        radius: f32,
        hit: RayHit,
        callback: Option<RayCastCallback<'cb>>,
        watertight: bool,
    ) -> Self {
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
            index[2 * i] = usize::from(idot_axis[i] < 0.0);
            index[2 * i + 1] = 1 - index[2 * i];
            index[2 * i] += 2 * i;
            index[2 * i + 1] += 2 * i;
        }
        RayCastData {
            tree,
            callback,
            ray: BvhRay {
                origin,
                direction: dir,
                radius,
                isect_precalc: watertight.then(|| IsectRayPrecalc::new(dir)),
            },
            ray_dot_axis,
            idot_axis,
            index,
            hit,
        }
    }

    /// `ray_nearest_hit`
    fn nearest_hit(&self, bv: &[f32; BV]) -> f32 {
        let mut low = 0.0f32;
        let mut upper = self.hit.dist;
        let (o, r) = (self.ray.origin, self.ray.radius);
        for i in 0..3 {
            let (bmin, bmax) = (bv[2 * i], bv[2 * i + 1]);
            if self.ray_dot_axis[i] == 0.0 {
                if o[i] < bmin - r || o[i] > bmax + r {
                    return f32::MAX;
                }
            } else {
                let ll = (bmin - r - o[i]) / self.ray_dot_axis[i];
                let lu = (bmax + r - o[i]) / self.ray_dot_axis[i];
                if self.ray_dot_axis[i] > 0.0 {
                    low = fmax(ll, low);
                    upper = fmin(lu, upper);
                } else {
                    low = fmax(lu, low);
                    upper = fmin(ll, upper);
                }
                if low > upper {
                    return f32::MAX;
                }
            }
        }
        low
    }

    /// `fast_ray_nearest_hit` (Fyffe's slab test; ignores the radius).
    fn fast_nearest_hit(&self, bv: &[f32; BV]) -> f32 {
        let o = self.ray.origin;
        let ix = &self.index;
        let id = &self.idot_axis;
        let t1x = (bv[ix[0]] - o[0]) * id[0];
        let t2x = (bv[ix[1]] - o[0]) * id[0];
        let t1y = (bv[ix[2]] - o[1]) * id[1];
        let t2y = (bv[ix[3]] - o[1]) * id[1];
        let t1z = (bv[ix[4]] - o[2]) * id[2];
        let t2z = (bv[ix[5]] - o[2]) * id[2];
        let d = self.hit.dist;
        if (t1x > t2y || t2x < t1y || t1x > t2z || t2x < t1z || t1y > t2z || t2y < t1z)
            || (t2x < 0.0 || t2y < 0.0 || t2z < 0.0)
            || (t1x > d || t1y > d || t1z > d)
        {
            return f32::MAX;
        }
        // `std::max({t1x, t1y, t1z})`: the first largest.
        let mut m = t1x;
        if m < t1y {
            m = t1y;
        }
        if m < t1z {
            m = t1z;
        }
        m
    }

    /// `dfs_raycast` (`all`: `dfs_raycast_all`, the distance reset after each callback).
    fn dfs(&mut self, node: u32, all: bool) {
        let t = self.tree;
        let bv = &t.bv[node as usize];
        let dist = if self.ray.radius == 0.0 {
            self.fast_nearest_hit(bv)
        } else {
            self.nearest_hit(bv)
        };
        if dist >= self.hit.dist {
            return;
        }
        if t.is_leaf(node) {
            let idx = t.index[node as usize];
            if all {
                let d = self.hit.dist;
                if let Some(cb) = self.callback.as_deref_mut() {
                    cb(idx, &self.ray, &mut self.hit);
                }
                self.hit.index = None;
                self.hit.dist = d;
            } else if let Some(cb) = self.callback.as_deref_mut() {
                cb(idx, &self.ray, &mut self.hit);
            } else {
                self.hit.index = Some(idx);
                self.hit.dist = dist;
                let (o, dir) = (self.ray.origin, self.ray.direction);
                self.hit.co = [
                    o[0] + dir[0] * dist,
                    o[1] + dir[1] * dist,
                    o[2] + dir[2] * dist,
                ];
            }
            return;
        }
        let n = t.node_num[node as usize] as usize;
        let forward = self.ray_dot_axis[t.main_axis[node as usize] as usize] > 0.0;
        for k in 0..n {
            let i = if forward { k } else { n - 1 - k };
            self.dfs(t.child(node, i), all);
        }
    }
}
