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
//! changes); frap builds them for every call (deviations D-147).

/// `KD_THRESH` (`ED_mesh_mirror_spatial_table_lookup`) and `BM_SEARCH_MAXDIST_MIRR`.
pub const KD_THRESH: f32 = 0.00002;

/// `KDTreeNode` (`BLI_kdtree_types.hh`): `kd_node_unset` children are [`KD_UNSET`].
#[derive(Clone, Copy)]
struct KdNode {
    left: u32,
    right: u32,
    co: [f32; 3],
    index: usize,
    d: usize,
}

/// `kd_node_unset`.
const KD_UNSET: u32 = u32::MAX;

/// `detail::kdtree_balance`: the median of `nodes` along `axis` becomes the subtree root
/// (quick-select around the median), its halves the children, the axis cycling X, Y, Z.
/// `ofs` is the offset of `nodes` in the whole node array.
fn kd_balance(nodes: &mut [KdNode], axis: usize, ofs: usize) -> u32 {
    let n = nodes.len();
    if n == 0 {
        return KD_UNSET;
    }
    if n == 1 {
        return ofs as u32;
    }
    let (mut left, mut right) = (0usize, n - 1);
    let median = n / 2;
    while right > left {
        let co = nodes[right].co[axis];
        let mut i = left.wrapping_sub(1);
        let mut j = right;
        loop {
            // `while (nodes[++i].co[axis] < co)`, `while (nodes[--j].co[axis] > co && j > left)`.
            i = i.wrapping_add(1);
            while nodes[i].co[axis] < co {
                i += 1;
            }
            j -= 1;
            while nodes[j].co[axis] > co && j > left {
                j -= 1;
            }
            if i >= j {
                break;
            }
            nodes.swap(i, j);
        }
        nodes.swap(i, right);
        if i >= median {
            right = i - 1;
        }
        if i <= median {
            left = i + 1;
        }
    }
    let next = (axis + 1) % 3;
    let (lo, rest) = nodes.split_at_mut(median);
    let (mid, hi) = rest.split_at_mut(1);
    mid[0].d = axis;
    mid[0].left = kd_balance(lo, next, ofs);
    mid[0].right = kd_balance(hi, next, ofs + median + 1);
    (median + ofs) as u32
}

/// The spatial mirror table of a vertex array (`ED_mesh_mirror_spatial_table_begin`): a port
/// of Blender's KD-tree (`kdtree_insert` in vertex order, `kdtree_balance`), so that among
/// vertices at the same distance (coincident duplicates) the lookup answers the one Blender's
/// traversal reaches first.
///
/// Ported from blender/source/blender/blenlib/BLI_kdtree.hh (Blender 5.2.4).
pub struct SpatialTable<'a> {
    positions: &'a [[f32; 3]],
    nodes: Vec<KdNode>,
    root: u32,
}

impl<'a> SpatialTable<'a> {
    /// The table of `positions`.
    pub fn new(positions: &'a [[f32; 3]]) -> Self {
        let mut nodes: Vec<KdNode> = positions
            .iter()
            .enumerate()
            .map(|(index, &co)| KdNode {
                left: KD_UNSET,
                right: KD_UNSET,
                co,
                index,
                d: 0,
            })
            .collect();
        let root = kd_balance(&mut nodes, 0, 0);
        SpatialTable {
            positions,
            nodes,
            root,
        }
    }

    /// `kdtree_find_nearest`: the vertex nearest to `co` and its distance. A node replaces the
    /// best one only when strictly closer, in the traversal order of Blender's stack.
    fn nearest(&self, co: [f32; 3]) -> Option<(usize, f32)> {
        if self.root == KD_UNSET {
            return None;
        }
        let mut min_dist = f32::MAX;
        let mut min_node: Option<usize> = None;
        let mut stack: Vec<u32> = vec![self.root];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni as usize];
            let mut test = |min_dist: &mut f32| {
                let q = node.co;
                let (dx, dy, dz) = (q[0] - co[0], q[1] - co[1], q[2] - co[2]);
                let dist_sq = dx * dx + dy * dy + dz * dz;
                // `if (dist_sq >= min_dist) return`: a NaN distance is accepted.
                if matches!(
                    dist_sq.partial_cmp(min_dist),
                    Some(std::cmp::Ordering::Less) | None
                ) {
                    *min_dist = dist_sq;
                    min_node = Some(node.index);
                }
            };
            let cur = node.co[node.d] - co[node.d];
            if cur < 0.0 {
                if cur * cur < min_dist {
                    test(&mut min_dist);
                    if node.left != KD_UNSET {
                        stack.push(node.left);
                    }
                }
                if node.right != KD_UNSET {
                    stack.push(node.right);
                }
            } else {
                if cur * cur < min_dist {
                    test(&mut min_dist);
                    if node.right != KD_UNSET {
                        stack.push(node.right);
                    }
                }
                if node.left != KD_UNSET {
                    stack.push(node.left);
                }
            }
        }
        min_node.map(|i| (i, min_dist.sqrt()))
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

    /// Blender 5.2.4 `mathutils.kdtree` (the same `kdtree_find_nearest`) on 200 points of a
    /// 5x3x2 grid, queried at every point mirrored along X: with so many coincident points
    /// the answer is the one the traversal reaches first, not the lowest index.
    #[test]
    fn spatial_table_matches_blender_kdtree_ties() {
        let mut s: u64 = 12345;
        let mut rnd = || {
            s = (s * 1103515245 + 12345) % (1 << 31);
            s
        };
        let pts: Vec<[f32; 3]> = (0..200)
            .map(|_| {
                let x = (rnd() % 5) as f32 - 2.0;
                let y = (rnd() % 3) as f32;
                let z = (rnd() % 2) as f32;
                [x, y, z]
            })
            .collect();
        let t = SpatialTable::new(&pts);
        let got: Vec<usize> = pts
            .iter()
            .map(|p| t.nearest([-p[0], p[1], p[2]]).unwrap().0)
            .collect();
        let want: [usize; 200] = [
            72, 107, 122, 121, 134, 13, 48, 85, 48, 43, 158, 111, 8, 103, 138, 69, 118, 139, 122,
            111, 158, 139, 72, 191, 36, 121, 36, 13, 8, 193, 8, 13, 48, 121, 36, 107, 26, 193, 36,
            13, 164, 121, 122, 69, 36, 85, 158, 193, 8, 167, 122, 183, 158, 139, 134, 103, 36, 183,
            36, 69, 184, 13, 72, 167, 26, 85, 164, 193, 138, 43, 72, 191, 0, 65, 116, 69, 118, 69,
            72, 85, 0, 111, 72, 13, 138, 65, 72, 191, 0, 121, 158, 167, 134, 65, 158, 13, 184, 183,
            16, 13, 122, 103, 48, 13, 72, 191, 138, 193, 16, 19, 48, 19, 122, 13, 48, 103, 138,
            183, 16, 139, 184, 121, 122, 103, 164, 167, 164, 191, 138, 85, 138, 183, 72, 69, 134,
            121, 48, 19, 116, 139, 48, 183, 0, 111, 118, 121, 122, 13, 8, 139, 8, 139, 8, 43, 122,
            85, 158, 121, 184, 13, 164, 121, 164, 193, 164, 85, 26, 183, 134, 139, 134, 121, 164,
            103, 16, 191, 36, 43, 36, 13, 0, 69, 138, 167, 158, 85, 134, 139, 116, 139, 164, 191,
            116, 107, 118, 69, 184, 139, 26, 191,
        ];
        assert_eq!(got, want);
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
