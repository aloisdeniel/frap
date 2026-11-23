// SPDX-License-Identifier: GPL-3.0-or-later
//! UV islands of the source mesh for face-corner (UV) transfer: faces grouped across edges
//! that are not seams, with each island's "inner cuts" (seam edges used twice by faces of the
//! same island, as the seam of an unwrapped cylinder).
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_mapping.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `face_edge_loop_islands_calc` (without bit-flag groups),
//! `mesh_calc_islands_loop_face_uv`, `BKE_mesh_calc_islands_loop_face_edgeseam` and
//! `BKE_mesh_loop_islands_add`; `bke::mesh::build_edge_to_face_map` ([`edge_to_face_map`]).

/// `MeshIslandStore` of `MISLAND_TYPE_LOOP` items, `MISLAND_TYPE_POLY` islands and
/// `MISLAND_TYPE_EDGE` inner cuts.
#[derive(Clone, Debug, Default)]
pub(crate) struct IslandStore {
    /// `items_to_islands`: the island of each face corner.
    pub items_to_islands: Vec<i32>,
    /// `islands`: the faces of each island, ascending.
    pub islands: Vec<Vec<i32>>,
    /// `innercuts`: the inner-cut edges of each island.
    pub innercuts: Vec<Vec<i32>>,
}

/// `bke::mesh::build_edge_to_face_map`: the faces of each edge, in face order.
pub(crate) fn edge_to_face_map(
    face_offsets: &[i32],
    corner_edges: &[i32],
    edges_num: usize,
) -> Vec<Vec<i32>> {
    let mut map = vec![Vec::new(); edges_num];
    for f in 0..face_offsets.len().saturating_sub(1) {
        for c in face_offsets[f]..face_offsets[f + 1] {
            map[corner_edges[c as usize] as usize].push(f as i32);
        }
    }
    map
}

/// `BKE_mesh_calc_islands_loop_face_edgeseam`: `None` when the mesh has no faces.
pub(crate) fn calc_islands_loop_face_edgeseam(
    edges_num: usize,
    uv_seams: Option<&[bool]>,
    face_offsets: &[i32],
    corner_edges: &[i32],
) -> Option<IslandStore> {
    let faces_num = face_offsets.len().saturating_sub(1);
    if faces_num == 0 {
        return None;
    }
    let edge_face_map = edge_to_face_map(face_offsets, corner_edges, edges_num);
    let is_boundary = |e: usize| uv_seams.is_some_and(|s| s[e]);

    // face_edge_loop_islands_calc
    let mut face_groups = vec![0i32; faces_num];
    let mut face_stack = vec![0usize; faces_num];
    let mut edge_boundaries = vec![false; edges_num];
    let mut num_edge_boundaries = 0usize;
    let mut tot_group = 0i32;
    let mut face_prev = 0usize;
    while let Some(start) = (face_prev..faces_num).find(|&f| face_groups[f] == 0) {
        tot_group += 1;
        let group = tot_group;
        face_prev = start + 1;
        face_groups[start] = group;
        let (mut curr, mut end) = (0usize, 0usize);
        face_stack[end] = start;
        end += 1;
        while curr != end {
            let face = face_stack[curr];
            curr += 1;
            for loop_ in face_offsets[face]..face_offsets[face + 1] {
                let edge = corner_edges[loop_ as usize] as usize;
                if !is_boundary(edge) {
                    for &p in &edge_face_map[edge] {
                        if face_groups[p as usize] == 0 {
                            face_groups[p as usize] = group;
                            face_stack[end] = p as usize;
                            end += 1;
                        }
                    }
                } else if !edge_boundaries[edge] {
                    edge_boundaries[edge] = true;
                    num_edge_boundaries += 1;
                }
            }
        }
    }

    // mesh_calc_islands_loop_face_uv
    let mut store = IslandStore {
        items_to_islands: vec![0; corner_edges.len()],
        islands: Vec::new(),
        innercuts: Vec::new(),
    };
    let mut edge_boundary_count = vec![0u8; edges_num];
    for grp in 1..=tot_group {
        let mut face_indices = Vec::new();
        let mut loop_indices = Vec::new();
        let mut innercuts = Vec::new();
        if num_edge_boundaries != 0 {
            edge_boundary_count.fill(0);
        }
        for f in 0..faces_num {
            if face_groups[f] != grp {
                continue;
            }
            face_indices.push(f as i32);
            for c in face_offsets[f]..face_offsets[f + 1] {
                let e = corner_edges[c as usize] as usize;
                loop_indices.push(c);
                if num_edge_boundaries != 0 && edge_boundaries[e] && edge_boundary_count[e] < 2 {
                    edge_boundary_count[e] += 1;
                    if edge_boundary_count[e] == 2 {
                        innercuts.push(e as i32);
                    }
                }
            }
        }
        // BKE_mesh_loop_islands_add
        let island = store.islands.len() as i32;
        for &l in loop_indices.iter().rev() {
            store.items_to_islands[l as usize] = island;
        }
        store.islands.push(face_indices);
        store.innercuts.push(innercuts);
    }
    Some(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seams_split_islands_and_mark_inner_cuts() {
        // A strip of three quads 0-1-2 closed into a ring (quad 2 shares edge 0 with quad 0),
        // the ring seam on edge 0 and a seam between quads 1 and 2 (edge 2).
        // Quads: q0 edges (0,1,..), q1 edges (1,2,..), q2 edges (2,0,..).
        let face_offsets = [0, 4, 8, 12];
        let corner_edges = [0, 3, 1, 4, 1, 5, 2, 6, 2, 7, 0, 8];
        let mut seams = vec![false; 9];
        seams[0] = true;
        let s =
            calc_islands_loop_face_edgeseam(9, Some(&seams), &face_offsets, &corner_edges).unwrap();
        assert_eq!(s.islands, vec![vec![0, 1, 2]]);
        assert_eq!(s.innercuts, vec![vec![0]]);
        seams[2] = true;
        let s =
            calc_islands_loop_face_edgeseam(9, Some(&seams), &face_offsets, &corner_edges).unwrap();
        assert_eq!(s.islands, vec![vec![0, 1], vec![2]]);
        assert!(s.innercuts.iter().all(Vec::is_empty));
        assert_eq!(&s.items_to_islands[8..], &[1, 1, 1, 1]);
        assert!(calc_islands_loop_face_edgeseam(0, None, &[0], &[]).is_none());
    }
}
