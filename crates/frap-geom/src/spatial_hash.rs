// SPDX-License-Identifier: GPL-3.0-or-later
//! Uniform-grid spatial hash for vertex welding (glTF import `merge_vertices`, WELD-style
//! merges). Deterministic: queries return indices in ascending order regardless of hash
//! iteration order. For Blender-identical *survivor selection* of `remove_doubles`, use
//! [`crate::kdtree::find_doubles_by_distance`] instead.

use rustc_hash::FxHashMap;

use crate::normals::V3;

/// Points bucketed in cubic cells of edge `cell`.
#[derive(Clone, Debug)]
pub struct SpatialHash {
    cell: f32,
    inv_cell: f32,
    buckets: FxHashMap<[i64; 3], Vec<u32>>,
    points: Vec<V3>,
}

impl SpatialHash {
    /// Empty hash with the given cell size (clamped to a tiny positive value).
    pub fn new(cell: f32) -> SpatialHash {
        let cell = if cell > 1e-12 { cell } else { 1e-12 };
        SpatialHash {
            cell,
            inv_cell: 1.0 / cell,
            buckets: FxHashMap::default(),
            points: Vec::new(),
        }
    }

    /// Cell edge length.
    pub fn cell_size(&self) -> f32 {
        self.cell
    }

    fn key(&self, co: V3) -> [i64; 3] {
        co.map(|c| (c * self.inv_cell).floor() as i64)
    }

    /// Adds a point; returns its index (insertion order).
    pub fn insert(&mut self, co: V3) -> u32 {
        let i = self.points.len() as u32;
        self.points.push(co);
        let k = self.key(co);
        self.buckets.entry(k).or_default().push(i);
        i
    }

    /// Indices of all points within `dist` (inclusive) of `co`, ascending.
    pub fn query(&self, co: V3, dist: f32) -> Vec<u32> {
        let lo = self.key([co[0] - dist, co[1] - dist, co[2] - dist]);
        let hi = self.key([co[0] + dist, co[1] + dist, co[2] + dist]);
        let d2 = dist * dist;
        let mut out = Vec::new();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    if let Some(b) = self.buckets.get(&[x, y, z]) {
                        for &i in b {
                            let p = self.points[i as usize];
                            let d = [p[0] - co[0], p[1] - co[1], p[2] - co[2]];
                            if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= d2 {
                                out.push(i);
                            }
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }
}

/// Welds points closer than `dist`: returns, for every point, the index of the first
/// (lowest-index) earlier point within `dist` that is itself a survivor, or its own index.
/// Survivors keep their order, so `remap` followed by compaction is stable.
pub fn weld_first_wins(points: &[V3], dist: f32) -> Vec<u32> {
    let mut hash = SpatialHash::new(if dist > 0.0 { dist } else { 1e-6 });
    let mut survivors_hash = SpatialHash::new(hash.cell_size());
    let mut survivor_ids: Vec<u32> = Vec::new();
    let mut out = Vec::with_capacity(points.len());
    for (i, &p) in points.iter().enumerate() {
        hash.insert(p);
        let near = survivors_hash.query(p, dist);
        if let Some(&s) = near.first() {
            out.push(survivor_ids[s as usize]);
        } else {
            survivors_hash.insert(p);
            survivor_ids.push(i as u32);
            out.push(i as u32);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_matches_brute_force() {
        let mut h = SpatialHash::new(0.25);
        let pts: Vec<V3> = (0..200)
            .map(|i| {
                let f = i as f32;
                [
                    libm::sinf(f * 0.37),
                    libm::cosf(f * 0.91),
                    libm::sinf(f * 1.7) * 0.5,
                ]
            })
            .collect();
        for p in &pts {
            h.insert(*p);
        }
        let probe = [0.1, -0.2, 0.05];
        let want: Vec<u32> = pts
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let d = [p[0] - probe[0], p[1] - probe[1], p[2] - probe[2]];
                d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= 0.3 * 0.3
            })
            .map(|(i, _)| i as u32)
            .collect();
        assert_eq!(h.query(probe, 0.3), want);
    }

    #[test]
    fn weld_keeps_first() {
        let pts = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1e-5],
            [1.0, 1e-5, 0.0],
        ];
        assert_eq!(weld_first_wins(&pts, 1e-4), vec![0, 1, 0, 1]);
    }
}
