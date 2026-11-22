// SPDX-License-Identifier: GPL-3.0-or-later
//! k-DOP bounding-volume hierarchy, to be ported from
//! `blender/source/blender/blenlib/intern/BLI_kdopbvh.cc` (Blender 5.2.4): the same tree
//! (`tree_type` children per node, `axis` = 6/8/14/18/26 k-DOP axes, `epsilon` inflation),
//! the same build order and traversal, so queries visit and tie-break primitives as Blender's
//! `BLI_bvhtree_*` do.
//!
//! M8 W0 contract (decision log M8W0-8): signatures only. WP 8.5 owns this file and
//! implements it (its bone-heat visibility rays, `heat_ray_source_visible`); WP 8.6 uses
//! [`BvhTree::overlap`] for the FLOAT solver's face pairs *(soft)*, and the W3 modifiers
//! (SHRINKWRAP, DATA_TRANSFER, VERTEX_WEIGHT_PROXIMITY) its nearest and ray queries.

use crate::normals::V3;

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

/// `BVHTree` (`BLI_bvhtree_new(maxsize, epsilon, tree_type, axis)`).
#[derive(Clone, Debug)]
pub struct BvhTree {
    max_size: usize,
    epsilon: f32,
    tree_type: u8,
    axis: u8,
}

impl BvhTree {
    /// `BLI_bvhtree_new`: room for `max_size` primitives, bounds inflated by `epsilon`,
    /// `tree_type` children per node (2..=32), `axis` k-DOP axes (6, 8, 14, 18 or 26).
    pub fn new(max_size: usize, epsilon: f32, tree_type: u8, axis: u8) -> BvhTree {
        BvhTree {
            max_size,
            epsilon,
            tree_type,
            axis,
        }
    }

    /// `BLI_bvhtree_insert`: primitive `index` bounded by the points `co`.
    pub fn insert(&mut self, index: usize, co: &[V3]) {
        let _ = (
            index,
            co,
            self.max_size,
            self.epsilon,
            self.tree_type,
            self.axis,
        );
        todo!("WP 8.5: port BLI_bvhtree_insert")
    }

    /// `BLI_bvhtree_balance`: builds the tree once every primitive is inserted.
    pub fn balance(&mut self) {
        todo!("WP 8.5: port BLI_bvhtree_balance")
    }

    /// `BLI_bvhtree_overlap`: the pairs `(self_index, other_index)` whose bounds overlap and
    /// that `exact` accepts (`BVHTree_OverlapCallback`), in Blender's traversal order.
    pub fn overlap(
        &self,
        other: &BvhTree,
        exact: &mut dyn FnMut(usize, usize) -> bool,
    ) -> Vec<(usize, usize)> {
        let _ = (other, exact);
        todo!("WP 8.5: port BLI_bvhtree_overlap")
    }

    /// `BLI_bvhtree_find_nearest`: the nearest primitive to `co` within `sqrt(dist_sq)`;
    /// `exact(index, co, &mut best)` refines a candidate (`BVHTree_NearestPointCallback`).
    pub fn find_nearest(
        &self,
        co: V3,
        dist_sq: f32,
        exact: &mut dyn FnMut(usize, V3, &mut Nearest),
    ) -> Option<Nearest> {
        let _ = (co, dist_sq, exact);
        todo!("WP 8.5: port BLI_bvhtree_find_nearest")
    }

    /// `BLI_bvhtree_ray_cast`: the first hit of the ray `origin + t·dir` (`t ≤ max_dist`,
    /// a sphere of `radius` when > 0); `exact(index, origin, dir, &mut best)` tests a
    /// candidate (`BVHTree_RayCastCallback`).
    pub fn ray_cast(
        &self,
        origin: V3,
        dir: V3,
        radius: f32,
        max_dist: f32,
        exact: &mut dyn FnMut(usize, V3, V3, &mut RayHit),
    ) -> Option<RayHit> {
        let _ = (origin, dir, radius, max_dist, exact);
        todo!("WP 8.5: port BLI_bvhtree_ray_cast")
    }

    /// `BLI_bvhtree_range_query`: every primitive whose bounds come within `radius` of `co`;
    /// `cb(index, co, dist_sq)` sees each one.
    pub fn range_query(&self, co: V3, radius: f32, cb: &mut dyn FnMut(usize, V3, f32)) -> usize {
        let _ = (co, radius, cb);
        todo!("WP 8.5: port BLI_bvhtree_range_query")
    }
}
