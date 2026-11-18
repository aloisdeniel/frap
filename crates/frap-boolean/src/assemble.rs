// SPDX-License-Identifier: GPL-3.0-or-later
//! The triangle soup a boolean returns, turned back into Blender faces: bad triangles removed
//! (`clean_meshgl`), the triangles of every input face merged into as few polygons as possible
//! (`merge_out_faces`), and redundant valence-2 vertices dissolved (`dissolve_valence2_verts`).
//!
//! Ported from blender/source/blender/geometry/intern/mesh_boolean_manifold.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `clean_meshgl`, `OutFace`, `get_face_groups`, `make_out_face`,
//! `SharedEdge`, `get_shared_edge_from_pair`, `get_shared_edges`, `is_legal_merge`,
//! `try_merge_out_face_pair`, `merge_out_face_pair`, `merge_out_faces`, `approx_in_line`,
//! `dissolve_valence2_verts` and `assemble_mesh_from_meshgl`. Element order is Blender's: the
//! output faces are the face groups in input-face order, each group's faces in triangle order.

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

/// `OutFace`: an output polygon (vertex indices into the assembly's positions) and the input
/// face (index in the concatenated input-face space) it lies on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutFace {
    /// Vertex ids.
    pub verts: SmallVec<[u32; 8]>,
    /// `face_id`: the input face (concatenated index).
    pub face_id: u32,
}

impl OutFace {
    /// `find_vert_index`.
    fn find_vert_index(&self, v: u32) -> Option<usize> {
        self.verts.iter().position(|&x| x == v)
    }
}

/// `MeshAssembly` after `assemble_mesh_from_meshgl`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assembly {
    /// Output vertex positions (dissolved vertices compressed out).
    pub positions: Vec<[f32; 3]>,
    /// Output faces in Blender's order.
    pub faces: Vec<OutFace>,
}

/// `clean_meshgl`: removes the triangles with an out-of-range or repeated vertex (with their
/// face ids).
pub fn clean_tris(verts_num: usize, tri_verts: &mut Vec<u32>, face_ids: &mut Vec<u32>) {
    let numtri = tri_verts.len() / 3;
    let bad = |t: &[u32]| {
        let n = verts_num as u32;
        t[0] >= n || t[1] >= n || t[2] >= n || t[0] == t[1] || t[0] == t[2] || t[1] == t[2]
    };
    if !(0..numtri).any(|t| bad(&tri_verts[3 * t..3 * t + 3])) {
        return;
    }
    let mut to = 0;
    for from in 0..numtri {
        let tri = [
            tri_verts[3 * from],
            tri_verts[3 * from + 1],
            tri_verts[3 * from + 2],
        ];
        if bad(&tri) {
            continue;
        }
        tri_verts[3 * to..3 * to + 3].copy_from_slice(&tri);
        face_ids[to] = face_ids[from];
        to += 1;
    }
    tri_verts.truncate(3 * to);
    face_ids.truncate(to);
}

/// `SharedEdge`: two "group edge" indices (triangle `e / 3`, edge `e % 3` of the group) with
/// the same vertices in opposite order; `v1 -> v2` runs along `e1`.
#[derive(Copy, Clone, Debug)]
struct SharedEdge {
    e1: usize,
    e2: usize,
    v1: u32,
    v2: u32,
}

/// `canon_shared_edge`: the canonical form has `v1 < v2`.
fn canon_shared_edge(e1: usize, e2: usize, v1: u32, v2: u32) -> SharedEdge {
    if v1 < v2 {
        SharedEdge { e1, e2, v1, v2 }
    } else {
        SharedEdge {
            e1: e2,
            e2: e1,
            v1: v2,
            v2: v1,
        }
    }
}

/// `get_shared_edge_from_pair`: the shared edge of two triangles (`e1 < 3 <= e2`).
fn get_shared_edge_from_pair(tri1: &OutFace, tri2: &OutFace) -> Option<SharedEdge> {
    for i1 in 0..3 {
        for i2 in 0..3 {
            let v1 = tri1.verts[i1];
            let v2 = tri2.verts[i2];
            if v1 == v2 {
                let v1_next = tri1.verts[(i1 + 1) % 3];
                let v2_prev = tri2.verts[(i2 + 2) % 3];
                if v1_next == v2_prev {
                    return Some(SharedEdge {
                        e1: i1,
                        e2: 3 + (i2 + 2) % 3,
                        v1,
                        v2: v1_next,
                    });
                }
                let v1_prev = tri1.verts[(i1 + 2) % 3];
                let v2_next = tri2.verts[(i2 + 1) % 3];
                if v1_prev == v2_next {
                    return Some(SharedEdge {
                        e1: (i1 + 2) % 3,
                        e2: 3 + i2,
                        v1: v1_prev,
                        v2: v1,
                    });
                }
            }
        }
    }
    None
}

/// `get_shared_edges`: every edge two triangles of the group share in opposite directions, in
/// discovery order.
fn get_shared_edges(faces: &[OutFace]) -> Vec<SharedEdge> {
    let mut ans = Vec::new();
    let mut edge_verts_to_tri: FxHashMap<(u32, u32), usize> = FxHashMap::default();
    for (face_index, f) in faces.iter().enumerate() {
        for i in 0..3 {
            let v1 = f.verts[i];
            let v2 = f.verts[(i + 1) % 3];
            let this_e = face_index * 3 + i;
            edge_verts_to_tri.insert((v1, v2), this_e);
            if let Some(&other_e) = edge_verts_to_tri.get(&(v2, v1)) {
                ans.push(canon_shared_edge(this_e, other_e, v1, v2));
            }
        }
    }
    ans
}

/// `is_legal_merge`: splicing `f1` and `f2` along `v1`-`v2` repeats no vertex.
fn is_legal_merge(f1: &OutFace, f2: &OutFace, v1: u32, v2: u32) -> bool {
    for &v in &f1.verts {
        if v != v1 && v != v2 && f2.find_vert_index(v).is_some() {
            return false;
        }
    }
    for &v in &f2.verts {
        if v != v1 && v != v2 && f1.find_vert_index(v).is_some() {
            return false;
        }
    }
    true
}

/// `try_merge_out_face_pair`: splices `f2` into `f1` along `se` (`v1 -> v2` in `f1`, the
/// reverse in `f2`) when that is legal.
fn try_merge_out_face_pair(f1: &mut OutFace, f2: &OutFace, se: &SharedEdge) -> bool {
    let f1_len = f1.verts.len();
    let f2_len = f2.verts.len();
    let (Some(i1), Some(i2)) = (f1.find_vert_index(se.v1), f2.find_vert_index(se.v2)) else {
        return false;
    };
    let i1_next = (i1 + 1) % f1_len;
    if !is_legal_merge(f1, f2, se.v1, se.v2) {
        return false;
    }
    // f1[0..=i1], f2[i2_next + 1 ..= i2_prev] (cyclic, `f2_len - 2` vertices), f1[i1_next..].
    let start = (i2 + 2) % f2_len;
    let insert: SmallVec<[u32; 8]> = (0..f2_len - 2)
        .map(|k| f2.verts[(start + k) % f2_len])
        .collect();
    f1.verts.insert_many(i1_next, insert);
    true
}

/// `merge_out_face_pair`: two triangles sharing an edge become the quad `b, c, a, d`.
fn merge_out_face_pair(faces: &mut Vec<OutFace>, can_dissolve: &dyn Fn(u32, u32) -> bool) {
    let Some(se) = get_shared_edge_from_pair(&faces[0], &faces[1]) else {
        return;
    };
    if !can_dissolve(se.v1, se.v2) {
        return;
    }
    let (va, vb) = (se.v1, se.v2);
    let vc = faces[0].verts[(se.e1 + 2) % 3];
    let vd = faces[1].verts[(se.e2 - 3 + 2) % 3];
    if vc == vd {
        return;
    }
    faces[0].verts = SmallVec::from_slice(&[vb, vc, va, vd]);
    faces.truncate(1);
}

/// `merge_out_faces`: dissolves as many shared edges of one input face's triangles as
/// possible while every face stays legal.
pub fn merge_out_faces(faces: &mut Vec<OutFace>) {
    merge_out_faces_with(faces, &|_, _| true);
}

/// [`merge_out_faces`] that only dissolves the shared edges `can_dissolve(v1, v2)` accepts
/// (the knife keeps its cut lines).
pub fn merge_out_faces_with(faces: &mut Vec<OutFace>, can_dissolve: &dyn Fn(u32, u32) -> bool) {
    if faces.len() <= 1 {
        return;
    }
    if faces.len() == 2 {
        merge_out_face_pair(faces, can_dissolve);
        return;
    }
    let shared_edges = get_shared_edges(faces);
    if shared_edges.is_empty() {
        return;
    }
    let mut merged_to: Vec<Option<usize>> = vec![None; faces.len()];
    let final_merged_to = |merged_to: &[Option<usize>], f: usize| {
        let mut f = f;
        while let Some(t) = merged_to[f] {
            f = t;
        }
        f
    };
    for se in &shared_edges {
        let f1 = final_merged_to(&merged_to, se.e1 / 3);
        let f2 = final_merged_to(&merged_to, se.e2 / 3);
        if f1 == f2 || !can_dissolve(se.v1, se.v2) {
            // `is_legal_merge` of a face with itself fails (every other vertex repeats).
            continue;
        }
        let other = faces[f2].clone();
        if try_merge_out_face_pair(&mut faces[f1], &other, se) {
            merged_to[f2] = Some(f1);
        }
    }
    let mut i = 0;
    faces.retain(|_| {
        let keep = merged_to[i].is_none();
        i += 1;
        keep
    });
}

/// `math::normalize` (`normalize_and_get_length`: zero below a squared length of `1e-35`).
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len_sq > 1.0e-35 {
        let len = len_sq.sqrt();
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0; 3]
    }
}

/// `approx_in_line`.
fn approx_in_line(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> bool {
    let a = normalize([p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]]);
    let b = normalize([p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]]);
    let cos_ang = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    (f64::from(cos_ang) - 1.0).abs() < 1e-4
}

/// `dissolve_valence2_verts`: removes the vertices that are valence 2 in the faces using them
/// (neighbours reversed between the two faces, never a triangle's) and lie between their
/// neighbours on a straight line; compresses `positions` and remaps the faces.
pub fn dissolve_valence2_verts(a: &mut Assembly) {
    let vnum = a.positions.len();
    let mut dissolve = vec![false; vnum];
    let mut neighbors: Vec<Option<(u32, u32)>> = vec![None; vnum];
    for face in &a.faces {
        let fsize = face.verts.len();
        for i in 0..fsize {
            let vprev = face.verts[(i + fsize - 1) % fsize];
            let v = face.verts[i];
            let vnext = face.verts[(i + 1) % fsize];
            match neighbors[v as usize] {
                None => {
                    neighbors[v as usize] = Some((vprev, vnext));
                    dissolve[v as usize] = fsize > 3;
                }
                Some((first, second)) => {
                    if fsize == 3 || !(vprev == second && vnext == first) {
                        dissolve[v as usize] = false;
                    }
                }
            }
        }
    }
    for face in &a.faces {
        let fsize = face.verts.len();
        let n = face.verts.iter().filter(|&&v| dissolve[v as usize]).count();
        if fsize - n < 3 {
            for &v in &face.verts {
                dissolve[v as usize] = false;
            }
        }
    }
    let mut any = false;
    for v in 0..vnum {
        if dissolve[v] {
            let (n0, n1) = neighbors[v].expect("a dissolvable vertex has neighbours");
            if approx_in_line(
                a.positions[n0 as usize],
                a.positions[v],
                a.positions[n1 as usize],
            ) {
                any = true;
            } else {
                dissolve[v] = false;
            }
        }
    }
    if !any {
        return;
    }
    let mut old_to_new = vec![u32::MAX; vnum];
    let mut kept = Vec::with_capacity(vnum);
    for v in 0..vnum {
        if !dissolve[v] {
            old_to_new[v] = kept.len() as u32;
            kept.push(a.positions[v]);
        }
    }
    a.positions = kept;
    for face in &mut a.faces {
        face.verts = face
            .verts
            .iter()
            .map(|&v| old_to_new[v as usize])
            .filter(|&v| v != u32::MAX)
            .collect();
    }
}

/// `assemble_mesh_from_meshgl` over a cleaned triangle soup: the triangles grouped by input
/// face (`get_face_groups`, groups in input-face order), each group merged, then valence-2
/// vertices dissolved.
pub fn assemble(
    positions: Vec<[f32; 3]>,
    tri_verts: &[u32],
    face_ids: &[u32],
    input_faces_num: usize,
) -> Assembly {
    let tris_num = tri_verts.len() / 3;
    let mut groups: Vec<SmallVec<[u32; 4]>> = vec![SmallVec::new(); input_faces_num];
    for (t, &fid) in face_ids.iter().enumerate().take(tris_num) {
        if (fid as usize) < input_faces_num {
            groups[fid as usize].push(t as u32);
        }
    }
    let mut faces = Vec::with_capacity(tris_num);
    for (gid, group) in groups.iter().enumerate() {
        let mut gf: Vec<OutFace> = group
            .iter()
            .map(|&t| {
                let k = 3 * t as usize;
                OutFace {
                    verts: SmallVec::from_slice(&tri_verts[k..k + 3]),
                    face_id: gid as u32,
                }
            })
            .collect();
        merge_out_faces(&mut gf);
        faces.extend(gf);
    }
    let mut a = Assembly { positions, faces };
    dissolve_valence2_verts(&mut a);
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: &[u32]) -> OutFace {
        OutFace {
            verts: SmallVec::from_slice(v),
            face_id: 0,
        }
    }

    #[test]
    fn two_triangles_make_blenders_quad() {
        // Triangles (0,1,2) and (0,2,3) of a quad: shared edge 2->0 in the first.
        let mut faces = vec![f(&[0, 1, 2]), f(&[0, 2, 3])];
        merge_out_faces(&mut faces);
        assert_eq!(faces.len(), 1);
        // a = 2, b = 0 (tri1 runs a -> b), c = 1, d = 3: quad b, c, a, d.
        assert_eq!(faces[0].verts.as_slice(), &[0, 1, 2, 3]);
    }

    #[test]
    fn fan_merges_into_one_polygon() {
        // A hexagon fanned from vertex 0.
        let mut faces = vec![f(&[0, 1, 2]), f(&[0, 2, 3]), f(&[0, 3, 4]), f(&[0, 4, 5])];
        merge_out_faces(&mut faces);
        assert_eq!(faces.len(), 1);
        let mut v = faces[0].verts.to_vec();
        let start = v.iter().position(|&x| x == 0).unwrap();
        v.rotate_left(start);
        assert_eq!(v, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn illegal_merge_is_refused() {
        // Two triangles sharing edge 1-2 and also vertex 3 elsewhere would repeat 3.
        let a = f(&[1, 2, 3]);
        let b = f(&[2, 1, 3]);
        let se = canon_shared_edge(0, 3, 1, 2);
        let mut a2 = a.clone();
        assert!(!try_merge_out_face_pair(&mut a2, &b, &se));
        assert_eq!(a2, a);
    }

    #[test]
    fn clean_removes_degenerate_triangles() {
        let mut tv = vec![0, 1, 2, 0, 0, 1, 1, 2, 9, 2, 1, 0];
        let mut ids = vec![5, 6, 7, 8];
        clean_tris(3, &mut tv, &mut ids);
        assert_eq!(tv, vec![0, 1, 2, 2, 1, 0]);
        assert_eq!(ids, vec![5, 8]);
    }

    #[test]
    fn colinear_valence2_vertex_is_dissolved() {
        // Two quads sharing the split edge 1-4-2... vertex 4 is in the middle of edge 1-2 of
        // both faces (valence 2, straight).
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.5, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        // The outer vertices also belong to triangles, which are never dissolved.
        let mut a = Assembly {
            positions,
            faces: vec![
                f(&[0, 1, 4, 2, 3]),
                f(&[1, 5, 6, 2, 4]),
                f(&[3, 7, 0]),
                f(&[5, 8, 6]),
            ],
        };
        dissolve_valence2_verts(&mut a);
        assert_eq!(a.positions.len(), 8);
        assert_eq!(a.faces[0].verts.as_slice(), &[0, 1, 2, 3]);
        assert_eq!(a.faces[1].verts.as_slice(), &[1, 4, 5, 2]);
        assert_eq!(a.faces[3].verts.as_slice(), &[4, 7, 5]);
    }
}
