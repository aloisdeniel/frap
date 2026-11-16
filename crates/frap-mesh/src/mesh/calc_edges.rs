// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh_calc_edges`: builds the edge list from the faces (keeping or discarding existing edges)
//! and fills `.corner_edge`. Ported from
//! `blender/source/blender/blenkernel/intern/mesh_calc_edges.cc` (Blender 5.2), including its
//! hash-map partitioning: meshes with 1000 faces or more spread edges over several ordered
//! sets keyed by the low vertex index, which changes the resulting edge order. Blender sizes
//! that partition from the CPU thread count (`min(8, threads)` rounded down to a power of two);
//! frap fixes it at 8, the value on every machine with 8 or more hardware threads.

use std::collections::HashMap;
use std::sync::Arc;

use super::{AttrData, Attribute, Domain, Mesh};

/// Number of hash maps for meshes with >= 1000 faces (see module docs).
const PARALLEL_MAPS_LARGE: usize = 8;

/// `VectorSet<OrderedEdge>`: insertion-ordered set of `(low, high)` pairs.
#[derive(Default)]
struct EdgeSet {
    items: Vec<(i32, i32)>,
    index: HashMap<(i32, i32), usize>,
}

impl EdgeSet {
    fn add(&mut self, e: (i32, i32)) {
        if let std::collections::hash_map::Entry::Vacant(v) = self.index.entry(e) {
            v.insert(self.items.len());
            self.items.push(e);
        }
    }
    fn index_of(&self, e: (i32, i32)) -> usize {
        self.index[&e]
    }
    fn contains(&self, e: (i32, i32)) -> bool {
        self.index.contains_key(&e)
    }
}

#[inline]
fn ordered(a: i32, b: i32) -> (i32, i32) {
    if a < b { (a, b) } else { (b, a) }
}

impl Mesh {
    /// `BKE_mesh_calc_edges(mesh, keep_existing_edges, select_new_edges)`: derives the edges of
    /// every face (new edges as `(low, high)` vertex pairs, after the kept ones), rewrites
    /// `.corner_edge`, re-creates `.edge_verts` (and `.select_edge` when `select_new_edges`) at
    /// the end of the attribute list, and remaps the other edge attributes (new edges get the
    /// default value).
    pub fn calc_edges(&mut self, keep_existing_edges: bool, select_new_edges: bool) {
        let edges_num = self.edges().len();
        let corners_num = self.corners_num();
        if edges_num == 0 && corners_num == 0 {
            return;
        }
        if corners_num == 0 && !keep_existing_edges {
            let attrs = self.attributes_mut();
            let names: Vec<_> = attrs
                .iter()
                .filter(|a| a.domain == Domain::Edge)
                .map(|a| a.name.clone())
                .collect();
            for n in names {
                attrs.remove(&n);
            }
            return;
        }
        let faces_num = self.faces_num();
        let maps_n = if faces_num < 1000 {
            1
        } else {
            PARALLEL_MAPS_LARGE
        };
        let mask = (maps_n - 1) as i32;
        let map_of = |e: (i32, i32)| (e.0 & mask) as usize;
        let mut maps: Vec<EdgeSet> = (0..maps_n).map(|_| EdgeSet::default()).collect();
        let original: Vec<[i32; 2]> = self.edges().to_vec();
        let mut prefix_sizes = vec![0usize; maps_n];
        if keep_existing_edges {
            for e in &original {
                let o = ordered(e[0], e[1]);
                maps[map_of(o)].add(o);
            }
            for (m, s) in maps.iter().zip(prefix_sizes.iter_mut()) {
                *s = m.items.len();
            }
        }
        let original_unique: usize = prefix_sizes.iter().sum();
        let distinct = original_unique == edges_num;
        if corners_num == 0 && keep_existing_edges && distinct {
            return;
        }
        let face_offsets: Vec<i32> = self.face_offsets().to_vec();
        let corner_verts: Vec<i32> = self.corner_verts().to_vec();
        for w in face_offsets.windows(2) {
            let (s, e) = (w[0] as usize, w[1] as usize);
            for c in s..e {
                let prev = if c == s { e - 1 } else { c - 1 };
                let o = ordered(corner_verts[prev], corner_verts[c]);
                maps[map_of(o)].add(o);
            }
        }
        let mut offsets = Vec::with_capacity(maps_n + 1);
        let mut acc = 0usize;
        for m in &maps {
            offsets.push(acc);
            acc += m.items.len();
        }
        offsets.push(acc);
        let mut prefix_offsets = Vec::with_capacity(maps_n + 1);
        let mut acc2 = 0usize;
        for &s in &prefix_sizes {
            prefix_offsets.push(acc2);
            acc2 += s;
        }
        prefix_offsets.push(acc2);
        let result_num = acc;
        let no_new_edges = result_num == original_unique;

        // `update_edge_indices_in_face_loops`: map entry index per corner (next-vertex edge).
        let map_entry_of_corner = |maps: &[EdgeSet], c: usize, s: usize, e: usize| -> usize {
            let next = if c + 1 == e { s } else { c + 1 };
            let o = ordered(corner_verts[next], corner_verts[c]);
            let m = map_of(o);
            offsets[m] + maps[m].index_of(o)
        };
        let corner_map_entries = |maps: &[EdgeSet]| -> Vec<usize> {
            let mut out = vec![0usize; corners_num];
            for w in face_offsets.windows(2) {
                let (s, e) = (w[0] as usize, w[1] as usize);
                for (c, slot) in out.iter_mut().enumerate().take(e).skip(s) {
                    *slot = map_entry_of_corner(maps, c, s, e);
                }
            }
            out
        };

        if keep_existing_edges && distinct && no_new_edges {
            let mut entry_to_result = vec![0usize; result_num];
            let mut iters = vec![0usize; maps_n];
            for (i, e) in original.iter().enumerate() {
                let m = map_of(ordered(e[0], e[1]));
                entry_to_result[offsets[m] + iters[m]] = i;
                iters[m] += 1;
            }
            let corner_edges: Vec<i32> = corner_map_entries(&maps)
                .into_iter()
                .map(|x| entry_to_result[x] as i32)
                .collect();
            self.write_corner_edges(corner_edges);
            return;
        }

        let mut edge_verts = vec![[-1i32, -1]; result_num];
        let src_to_dst: Vec<usize>;
        let new_start: usize;
        let mut entry_to_result = vec![usize::MAX; result_num];
        if keep_existing_edges {
            new_start = original_unique;
            src_to_dst = if distinct {
                (0..original_unique).collect()
            } else {
                first_distinct(&original, (0..edges_num).collect())
            };
            for (dst, &src) in src_to_dst.iter().enumerate() {
                edge_verts[dst] = original[src];
            }
            if distinct {
                let mut iters = vec![0usize; maps_n];
                for (i, e) in original.iter().enumerate() {
                    let m = map_of(ordered(e[0], e[1]));
                    entry_to_result[offsets[m] + iters[m]] = i;
                    iters[m] += 1;
                }
            } else {
                for (dst, &src) in src_to_dst.iter().enumerate() {
                    let o = ordered(original[src][0], original[src][1]);
                    let m = map_of(o);
                    entry_to_result[offsets[m] + maps[m].index_of(o)] = dst;
                }
            }
            if !no_new_edges {
                for m in 0..maps_n {
                    let prefix = prefix_sizes[m];
                    let start_pos = offsets[m] - prefix_offsets[m];
                    for k in prefix..maps[m].items.len() {
                        entry_to_result[offsets[m] + k] = new_start + start_pos + (k - prefix);
                    }
                }
            }
            for m in 0..maps_n {
                let prefix = prefix_sizes[m];
                let start_pos = offsets[m] - prefix_offsets[m];
                for (k, &(a, b)) in maps[m].items.iter().enumerate().skip(prefix) {
                    edge_verts[new_start + start_pos + (k - prefix)] = [a, b];
                }
            }
        } else if edges_num != 0 {
            let in_maps: Vec<usize> = (0..edges_num)
                .filter(|&i| {
                    let o = ordered(original[i][0], original[i][1]);
                    maps[map_of(o)].contains(o)
                })
                .collect();
            src_to_dst = first_distinct(&original, in_maps);
            let old_n = src_to_dst.len();
            new_start = old_n;
            if !src_to_dst.is_empty() {
                let mut is_new = vec![1usize; result_num];
                for &src in &src_to_dst {
                    let o = ordered(original[src][0], original[src][1]);
                    let m = map_of(o);
                    is_new[offsets[m] + maps[m].index_of(o)] = 0;
                }
                let mut acc = old_n;
                for (x, slot) in is_new.iter().zip(entry_to_result.iter_mut()) {
                    *slot = acc;
                    acc += *x;
                }
                for (dst, &src) in src_to_dst.iter().enumerate() {
                    let o = ordered(original[src][0], original[src][1]);
                    let m = map_of(o);
                    entry_to_result[offsets[m] + maps[m].index_of(o)] = dst;
                }
                for (dst, &src) in src_to_dst.iter().enumerate() {
                    edge_verts[dst] = original[src];
                }
                for m in 0..maps_n {
                    for (k, &(a, b)) in maps[m].items.iter().enumerate() {
                        edge_verts[entry_to_result[offsets[m] + k]] = [a, b];
                    }
                }
            } else {
                for (i, slot) in entry_to_result.iter_mut().enumerate() {
                    *slot = i;
                }
                for m in 0..maps_n {
                    for (k, &(a, b)) in maps[m].items.iter().enumerate() {
                        edge_verts[offsets[m] + k] = [a, b];
                    }
                }
            }
        } else {
            src_to_dst = Vec::new();
            new_start = 0;
            for (i, slot) in entry_to_result.iter_mut().enumerate() {
                *slot = i;
            }
            for m in 0..maps_n {
                for (k, &(a, b)) in maps[m].items.iter().enumerate() {
                    edge_verts[offsets[m] + k] = [a, b];
                }
            }
        }
        let corner_edges: Vec<i32> = corner_map_entries(&maps)
            .into_iter()
            .map(|x| entry_to_result[x] as i32)
            .collect();
        let new_count = result_num - new_start;

        // Remap the remaining edge attributes; `.edge_verts` is re-created at the end.
        self.write_corner_edges(corner_edges);
        let attrs = self.attributes_mut();
        attrs.remove(".edge_verts");
        for a in attrs.attrs.iter_mut().filter(|a| a.domain == Domain::Edge) {
            a.data = a.data.gather_with_default(&src_to_dst, result_num);
        }
        attrs.insert(Attribute {
            name: ".edge_verts".into(),
            domain: Domain::Edge,
            data: AttrData::Int32x2(Arc::new(edge_verts)),
        });
        if select_new_edges {
            attrs.remove(".select_edge");
            let values: Vec<bool> = (0..result_num)
                .map(|i| i >= result_num - new_count)
                .collect();
            attrs.insert(Attribute {
                name: ".select_edge".into(),
                domain: Domain::Edge,
                data: AttrData::Bool(Arc::new(values)),
            });
        }
    }

    /// Writes `.corner_edge`, creating it at the end when missing (`attributes.add` keeps an
    /// existing attribute's position).
    fn write_corner_edges(&mut self, corner_edges: Vec<i32>) {
        self.attributes_mut().insert(Attribute {
            name: ".corner_edge".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_edges)),
        });
    }
}

/// `mask_first_distinct_edges`: of `candidates` (ascending), the first occurrence of every
/// distinct unordered edge.
fn first_distinct(edges: &[[i32; 2]], candidates: Vec<usize>) -> Vec<usize> {
    let mut first: HashMap<(i32, i32), usize> = HashMap::new();
    for &i in &candidates {
        first.entry(ordered(edges[i][0], edges[i][1])).or_insert(i);
    }
    candidates
        .into_iter()
        .filter(|&i| first[&ordered(edges[i][0], edges[i][1])] == i)
        .collect()
}
