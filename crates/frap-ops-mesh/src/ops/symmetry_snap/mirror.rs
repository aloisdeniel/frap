// SPDX-License-Identifier: GPL-3.0-or-later
//! The Edit Mode mirror cache of `EDBM_verts_mirror_cache_begin_ex` as `mesh.symmetry_snap`
//! calls it (`use_self`, `use_select`, no hidden check, any axis, a distance limit): the
//! nearest vertex to the mirrored position in Blender's KD-tree (`BLI_kdtree_3d`, ties
//! resolved by its traversal order), or the topological mirror (`ED_mesh_mirrtopo_init`).
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_utils.cc, mesh_mirror.cc and
//! blender/source/blender/blenlib/BLI_kdtree.hh (Blender 5.2.4, GPL-2.0-or-later). The KD-tree
//! and the topology hash are the same ports as `frap-ops-object`'s `support/weights/mirror.rs`
//! (WP 8.3.7), which a sibling L10 crate cannot reach.

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

/// A KD-tree of a vertex array: a port
/// of Blender's KD-tree (`kdtree_insert` in vertex order, `kdtree_balance`), so that among
/// vertices at the same distance (coincident duplicates) the lookup answers the one Blender's
/// traversal reaches first.
///
/// Ported from blender/source/blender/blenlib/BLI_kdtree.hh (Blender 5.2.4).
pub(crate) struct SpatialTable {
    nodes: Vec<KdNode>,
    root: u32,
}

impl SpatialTable {
    /// The table of `positions`.
    pub(crate) fn new(positions: &[[f32; 3]]) -> Self {
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
        SpatialTable { nodes, root }
    }

    /// `kdtree_find_nearest`: the vertex nearest to `co` and its distance. A node replaces the
    /// best one only when strictly closer, in the traversal order of Blender's stack.
    pub(crate) fn nearest(&self, co: [f32; 3]) -> Option<(usize, f32)> {
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
}

/// `ED_mesh_mirrtopo_init`: for every vertex, its topological mirror (itself for a centre
/// vertex, whose hash no other vertex shares), `None` when its hash is shared by more than two
/// vertices. `edges` are the vertex pairs of the mesh (or of the edit BMesh, in index order).
pub(crate) fn topology_table(verts: usize, edges: &[[usize; 2]]) -> Vec<Option<usize>> {
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

/// `EDBM_verts_mirror_cache_begin_ex(em, axis, use_self = true, use_select = true,
/// respecthide = false, use_topology, maxdist, r_index)`: for every selected vertex (index
/// order) its mirror's index goes into its own slot and its own index into the mirror's slot,
/// so a later vertex can overwrite an earlier answer; `None` where no mirror was found.
/// Unselected vertices that are nobody's mirror keep `None` (Blender leaves them
/// uninitialised; `symmetry_snap` never reads them).
pub(crate) fn mirror_cache(
    positions: &[[f32; 3]],
    edges: &[[usize; 2]],
    selected: &[bool],
    axis: usize,
    use_topology: bool,
    maxdist: f32,
) -> Vec<Option<usize>> {
    let n = positions.len();
    let maxdist_sq = maxdist * maxdist;
    let topo = use_topology.then(|| topology_table(n, edges));
    let tree = (!use_topology).then(|| SpatialTable::new(positions));
    let mut idx: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        if !selected[i] {
            continue;
        }
        let mirr = match (&topo, &tree) {
            (Some(t), _) => t[i],
            (None, Some(t)) => {
                let mut co = positions[i];
                co[axis] *= -1.0;
                t.nearest(co).and_then(|(j, _)| {
                    let q = positions[j];
                    let d = [co[0] - q[0], co[1] - q[1], co[2] - q[2]];
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < maxdist_sq).then_some(j)
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

    #[test]
    fn spatial_cache_pairs_mirrored_vertices_on_any_axis() {
        let p = [
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.5, 0.0, 0.0],
            [0.0, 0.01, 0.0],
        ];
        let sel = [true; 4];
        let m = mirror_cache(&p, &[], &sel, 1, false, 0.05);
        assert_eq!(m[0], Some(1));
        assert_eq!(m[1], Some(0));
        assert_eq!(m[2], Some(2));
        // Vertex 3 mirrors to (0, -0.01, 0): its nearest neighbour is itself (0.02 away).
        assert_eq!(m[3], Some(3));
        let m = mirror_cache(&p, &[], &sel, 1, false, 0.01);
        assert_eq!(m[3], None);
    }
}
