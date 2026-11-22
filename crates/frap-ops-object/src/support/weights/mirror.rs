// SPDX-License-Identifier: GPL-3.0-or-later
//! X-mirror vertex lookups of mirror editing: the spatial table (`ED_mesh_mirror_spatial_table_*`,
//! a vertex within `KD_THRESH` of the mirrored position), the topology table
//! (`ED_mesh_mirrtopo_init`: vertices paired by an edge-count hash refined until it stops
//! separating more vertices) and the Edit Mode cache of `EDBM_verts_mirror_cache_begin_ex`.
//!
//! Ported from blender/source/blender/editors/mesh/mesh_mirror.cc, meshtools.cc
//! (`mesh_get_x_mirror_vert`) and editmesh_utils.cc (Blender 5.2.4, GPL-2.0-or-later).
//!
//! Blender keeps the Object Mode tables in process-wide statics between operator calls (the
//! topology table is rebuilt only when the vertex or edge count or the Edit Mode state
//! changes); frap builds them for every call (deviations D-145).

use std::collections::HashMap;

/// `KD_THRESH` (`ED_mesh_mirror_spatial_table_lookup`) and `BM_SEARCH_MAXDIST_MIRR`.
pub const KD_THRESH: f32 = 0.00002;

/// The spatial mirror table of a vertex array (`ED_mesh_mirror_spatial_table_begin`).
///
/// Blender queries a KD-tree for the nearest vertex and accepts it within [`KD_THRESH`]; the
/// vertices are hashed in cubes of that size here, so only the 27 cubes around the query are
/// searched. The answer is the same (ties: the lowest index).
pub struct SpatialTable<'a> {
    positions: &'a [[f32; 3]],
    cells: HashMap<[i64; 3], Vec<usize>>,
}

impl<'a> SpatialTable<'a> {
    fn cell(p: [f32; 3]) -> [i64; 3] {
        // Saturating casts: huge or non-finite coordinates land in the outermost cells, where
        // the distance test still decides.
        let f = |x: f32| (x / KD_THRESH).floor() as i64;
        [f(p[0]), f(p[1]), f(p[2])]
    }

    /// The table of `positions`.
    pub fn new(positions: &'a [[f32; 3]]) -> Self {
        let mut cells: HashMap<[i64; 3], Vec<usize>> = HashMap::with_capacity(positions.len());
        for (j, q) in positions.iter().enumerate() {
            cells.entry(Self::cell(*q)).or_default().push(j);
        }
        SpatialTable { positions, cells }
    }

    /// The vertex nearest to `co` and its distance (`kdtree_find_nearest`), searching the
    /// cubes around `co` only (a farther vertex is never within the threshold).
    fn nearest(&self, co: [f32; 3]) -> Option<(usize, f32)> {
        let c = Self::cell(co);
        let mut best: Option<(usize, f32)> = None;
        for dx in -1i64..=1 {
            for dy in -1i64..=1 {
                for dz in -1i64..=1 {
                    let key = [
                        c[0].saturating_add(dx),
                        c[1].saturating_add(dy),
                        c[2].saturating_add(dz),
                    ];
                    for &j in self.cells.get(&key).into_iter().flatten() {
                        let q = self.positions[j];
                        let d = ((q[0] - co[0]).powi(2)
                            + (q[1] - co[1]).powi(2)
                            + (q[2] - co[2]).powi(2))
                        .sqrt();
                        if best.is_none_or(|(bj, bd)| d < bd || (d == bd && j < bj)) {
                            best = Some((j, d));
                        }
                    }
                }
            }
        }
        best
    }

    /// `ED_mesh_mirror_spatial_table_lookup(co)`: the vertex within [`KD_THRESH`] of `co`.
    pub fn lookup(&self, co: [f32; 3]) -> Option<usize> {
        self.nearest(co)
            .filter(|(_, d)| *d < KD_THRESH)
            .map(|(j, _)| j)
    }

    /// `mesh_get_x_mirror_vert_spatial(ob, nullptr, i)`: the vertex at vertex `i`'s position
    /// mirrored along local X.
    pub fn mirror_of(&self, i: usize) -> Option<usize> {
        let p = self.positions[i];
        self.lookup([-p[0], p[1], p[2]])
    }
}

/// `ED_mesh_mirrtopo_init`: for every vertex, its topological mirror (itself for a centre
/// vertex, whose hash no other vertex shares), `None` when its hash is shared by more than two
/// vertices. `edges` are the vertex pairs of the mesh (or of the edit BMesh, in index order).
pub fn topology_table(verts: usize, edges: &[[usize; 2]]) -> Vec<Option<usize>> {
    // `MirrTopoHash_t` is a 32-bit unsigned int; the sums wrap as in C.
    let mut hash = vec![0u32; verts];
    for e in edges {
        hash[e[0]] = hash[e[0]].wrapping_add(1);
        hash[e[1]] = hash[e[1]].wrapping_add(1);
    }
    let mut prev = hash.clone();
    let mut tot_unique_prev: i64 = -1;
    let mut tot_unique_edges_prev: i64 = -1;
    let mut topo_pass: u32 = 1;
    loop {
        let mut tot_unique_edges: i64 = 0;
        for e in edges {
            let (i1, i2) = (e[0], e[1]);
            hash[i1] = hash[i1].wrapping_add(prev[i2].wrapping_mul(topo_pass));
            hash[i2] = hash[i2].wrapping_add(prev[i1].wrapping_mul(topo_pass));
            tot_unique_edges += i64::from(hash[i1] != hash[i2]);
        }
        prev.copy_from_slice(&hash);
        prev.sort_unstable();
        let tot_unique: i64 = 1 + prev.windows(2).filter(|w| w[0] != w[1]).count() as i64;
        if tot_unique <= tot_unique_prev && tot_unique_edges <= tot_unique_edges_prev {
            break;
        }
        tot_unique_prev = tot_unique;
        tot_unique_edges_prev = tot_unique_edges;
        prev.copy_from_slice(&hash);
        topo_pass = topo_pass.wrapping_add(1);
    }
    // Pairs out of the sorted hashes: a hash held by two vertices pairs them, by one vertex
    // makes it a centre vertex. The order within a run does not matter.
    let mut pairs: Vec<(u32, usize)> = hash.iter().copied().zip(0..verts).collect();
    pairs.sort_unstable_by_key(|p| p.0);
    let mut lookup = vec![None; verts];
    let mut last = 0usize;
    for a in 1..=verts {
        if a == verts || pairs[a - 1].0 != pairs[a].0 {
            match a - last {
                2 => {
                    let (j, k) = (pairs[a - 1].1, pairs[a - 2].1);
                    lookup[j] = Some(k);
                    lookup[k] = Some(j);
                }
                1 => {
                    let j = pairs[a - 1].1;
                    lookup[j] = Some(j);
                }
                _ => {}
            }
            last = a;
        }
    }
    lookup
}

/// `mesh_get_x_mirror_vert(ob, nullptr, i, use_topology)` for every vertex of a mesh in
/// Object Mode.
pub fn object_mirror_map(
    positions: &[[f32; 3]],
    edges: &[[usize; 2]],
    use_topology: bool,
) -> Vec<Option<usize>> {
    if use_topology {
        return topology_table(positions.len(), edges);
    }
    let t = SpatialTable::new(positions);
    (0..positions.len()).map(|i| t.mirror_of(i)).collect()
}

/// `EDBM_verts_mirror_cache_begin(em, 0, use_self = true, use_select = false, respecthide =
/// false, use_topology)` followed by `EDBM_verts_mirror_get` for every vertex of an edit
/// BMesh (vertex index order): each vertex writes its mirror's index into its own slot and its
/// own index into the mirror's slot, in vertex order, so a later vertex can overwrite an
/// earlier answer, as in Blender.
pub fn edit_mirror_map(
    positions: &[[f32; 3]],
    edges: &[[usize; 2]],
    use_topology: bool,
) -> Vec<Option<usize>> {
    let n = positions.len();
    let topo = use_topology.then(|| topology_table(n, edges));
    let table = (!use_topology).then(|| SpatialTable::new(positions));
    let mut idx: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        let mirr = match (&topo, &table) {
            (Some(t), _) => t[i],
            (None, Some(t)) => {
                let p = positions[i];
                let co = [-p[0], p[1], p[2]];
                // `len_squared_v3v3(co, v_test->co) < maxdist_sq`.
                t.nearest(co).and_then(|(j, _)| {
                    let q = positions[j];
                    let d2 =
                        (co[0] - q[0]).powi(2) + (co[1] - q[1]).powi(2) + (co[2] - q[2]).powi(2);
                    (d2 < KD_THRESH * KD_THRESH).then_some(j)
                })
            }
            (None, None) => None,
        };
        match mirr {
            Some(m) => {
                idx[i] = Some(m);
                idx[m] = Some(i);
            }
            None => idx[i] = None,
        }
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of quads along X, three wide, with one extra vertex on the right end so the
    /// topology is not left-right symmetric only when `asym`.
    fn ladder() -> (Vec<[f32; 3]>, Vec<[usize; 2]>) {
        // 0 1 2 3   (y = 0)
        // 4 5 6 7   (y = 1)
        let mut p = Vec::new();
        for y in 0..2 {
            for x in 0..4 {
                p.push([x as f32 - 1.5, y as f32, 0.0]);
            }
        }
        let mut e = Vec::new();
        for y in 0..2 {
            for x in 0..3 {
                e.push([y * 4 + x, y * 4 + x + 1]);
            }
        }
        for x in 0..4 {
            e.push([x, x + 4]);
        }
        (p, e)
    }

    #[test]
    fn spatial_mirror_pairs_opposite_vertices() {
        let (p, e) = ladder();
        let m = object_mirror_map(&p, &e, false);
        assert_eq!(
            m,
            vec![
                Some(3),
                Some(2),
                Some(1),
                Some(0),
                Some(7),
                Some(6),
                Some(5),
                Some(4)
            ]
        );
        let mut q = p.clone();
        q[0][0] += 1e-3;
        let m = object_mirror_map(&q, &e, false);
        assert_eq!((m[0], m[3]), (None, None));
    }

    #[test]
    fn topology_mirror_needs_unique_hashes() {
        // The ladder is symmetric in X and in Y: every hash is shared by four vertices.
        let (p, e) = ladder();
        assert!(topology_table(p.len(), &e).iter().all(|x| x.is_none()));
        // A tail on the bottom row breaks the Y symmetry: X pairs remain.
        let mut p = p;
        let mut e = e;
        p.push([0.0, -1.0, 0.0]);
        p.push([0.0, -2.0, 0.0]);
        e.push([1, 8]);
        e.push([2, 8]);
        e.push([8, 9]);
        let t = topology_table(p.len(), &e);
        assert_eq!(t[0], Some(3));
        assert_eq!(t[1], Some(2));
        assert_eq!(t[4], Some(7));
        assert_eq!(t[8], Some(8));
        assert_eq!(t[9], Some(9));
    }

    #[test]
    fn edit_cache_lets_later_vertices_overwrite() {
        // Vertex 2 has no partner of its own, but vertex 0's mirror is vertex 1, and vertex 1's
        // mirror would be vertex 0 again.
        let p = vec![
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [0.0, 3.0, 0.0],
        ];
        let m = edit_mirror_map(&p, &[], false);
        assert_eq!(m, vec![Some(1), Some(0), None, Some(3)]);
    }
}
