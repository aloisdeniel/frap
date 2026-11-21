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
    pub(crate) fn find_vert_index(&self, v: u32) -> Option<usize> {
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
        frap_math::budget::check();
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
        frap_math::budget::check();
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
        frap_math::budget::check();
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
        frap_math::budget::check();
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
        frap_math::budget::check();
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
        frap_math::budget::check();
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
        frap_math::budget::check();
        if !dissolve[v] {
            old_to_new[v] = kept.len() as u32;
            kept.push(a.positions[v]);
        }
    }
    a.positions = kept;
    for face in &mut a.faces {
        frap_math::budget::check();
        face.verts = face
            .verts
            .iter()
            .map(|&v| old_to_new[v as usize])
            .filter(|&v| v != u32::MAX)
            .collect();
    }
}

/// Moves the bridge edges that merging left at an in-line vertex `v` (`line[v] = (a, b)`: a
/// corner `a, v, w` of a face of one input face's `group`, `v`-`w` shared with another face
/// of the group and not `is_cut`) to `a`: the ear `a, v, w` goes to the other face, where `v`
/// is then between its two cut-line neighbours, so it can be dissolved. An ear that is
/// not convex, holds another vertex of its face or would repeat `a` is left alone.
pub fn reroute_bridges(
    group: &mut [OutFace],
    line: &FxHashMap<u32, (u32, u32)>,
    pos: &dyn Fn(u32) -> [f64; 3],
    is_cut: &dyn Fn(u32, u32) -> bool,
) {
    if group.len() < 2 || line.is_empty() {
        return;
    }
    let mut cands: Vec<u32> = group
        .iter()
        .flat_map(|f| f.verts.iter().copied())
        .filter(|v| line.contains_key(v))
        .collect();
    cands.sort_unstable();
    cands.dedup();
    for v in cands {
        frap_math::budget::check();
        let (a, b) = line[&v];
        for _ in 0..2 * group.len() {
            let mut moved = false;
            let mut blocked = false;
            for fi in 0..group.len() {
                let f = &group[fi];
                let Some(i) = f.find_vert_index(v) else {
                    continue;
                };
                let k = f.verts.len();
                let (p, n) = (f.verts[(i + k - 1) % k], f.verts[(i + 1) % k]);
                let on = |x: u32| x == a || x == b;
                if on(p) && on(n) {
                    continue;
                }
                // `ear_first`: the face runs `a, v, w` (else `w, v, a`).
                let (ear_first, w) = match (on(p), on(n)) {
                    (true, false) => (true, n),
                    (false, true) => (false, p),
                    _ => {
                        blocked = true;
                        break;
                    }
                };
                let a_here = if ear_first { p } else { n };
                if is_cut(v, w) {
                    continue;
                }
                // The other face of the group with the edge `v`-`w` reversed.
                let other = group.iter().position(|g| {
                    let m = g.verts.len();
                    g.find_vert_index(w).is_some_and(|j| {
                        if ear_first {
                            g.verts[(j + 1) % m] == v
                        } else {
                            g.verts[(j + m - 1) % m] == v
                        }
                    })
                });
                let Some(oi) = other.filter(|&oi| oi != fi) else {
                    continue;
                };
                if k <= 3
                    || group[oi].find_vert_index(a_here).is_some()
                    || !is_ear(&group[fi].verts, i, pos)
                {
                    // Another corner of `v` may move instead.
                    continue;
                }
                group[fi].verts.remove(i);
                let g = &mut group[oi];
                let j = g.find_vert_index(v).expect("the other face has v");
                // `w, v` (ear first) becomes `w, a, v`; `v, w` becomes `v, a, w`.
                let at = if ear_first { j } else { j + 1 };
                g.verts.insert(at, a_here);
                moved = true;
                break;
            }
            if blocked || !moved {
                break;
            }
        }
    }
}

/// Whether the corner `i` of the planar polygon `verts` is convex and its triangle holds
/// no other vertex of the polygon.
fn is_ear(verts: &[u32], i: usize, pos: &dyn Fn(u32) -> [f64; 3]) -> bool {
    let k = verts.len();
    let pt = pos;
    // Newell normal; project along its largest axis.
    let mut nrm = [0.0f64; 3];
    for j in 0..k {
        let (c, d) = (pt(verts[j]), pt(verts[(j + 1) % k]));
        nrm[0] += (c[1] - d[1]) * (c[2] + d[2]);
        nrm[1] += (c[2] - d[2]) * (c[0] + d[0]);
        nrm[2] += (c[0] - d[0]) * (c[1] + d[1]);
    }
    let ax = nrm.map(f64::abs);
    let (u, w, sign) = if ax[0] >= ax[1] && ax[0] >= ax[2] {
        (1, 2, nrm[0].signum())
    } else if ax[1] >= ax[2] {
        (2, 0, nrm[1].signum())
    } else {
        (0, 1, nrm[2].signum())
    };
    let p2 = |x: u32| (pt(x)[u], pt(x)[w]);
    let orient = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        sign * ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0))
    };
    let (pa, pv, pn) = (
        p2(verts[(i + k - 1) % k]),
        p2(verts[i]),
        p2(verts[(i + 1) % k]),
    );
    let area = orient(pa, pv, pn);
    if area <= 0.0 {
        return false;
    }
    let tol = 1e-12 * area.abs().max(1e-300);
    for (j, &x) in verts.iter().enumerate() {
        if j == i || j == (i + 1) % k || j == (i + k - 1) % k {
            continue;
        }
        let q = p2(x);
        if orient(pa, pv, q) >= -tol && orient(pv, pn, q) >= -tol && orient(pn, pa, q) >= -tol {
            return false;
        }
    }
    true
}

/// The vertices (not `is_input`) with exactly two neighbours across edges between different
/// input faces, strictly between them on a straight line: `v -> (a, b)`.
fn in_line_vertices(
    positions: &[[f32; 3]],
    faces: &[OutFace],
    is_input: &dyn Fn(u32) -> bool,
) -> FxHashMap<u32, (u32, u32)> {
    let mut edge_face: FxHashMap<(u32, u32), u32> = FxHashMap::default();
    for f in faces {
        frap_math::budget::check();
        let k = f.verts.len();
        for i in 0..k {
            edge_face.insert((f.verts[i], f.verts[(i + 1) % k]), f.face_id);
        }
    }
    let mut nb: FxHashMap<u32, SmallVec<[u32; 4]>> = FxHashMap::default();
    for (&(u, w), &fid) in &edge_face {
        if edge_face.get(&(w, u)) == Some(&fid) {
            continue;
        }
        for (x, y) in [(u, w), (w, u)] {
            let n = nb.entry(x).or_default();
            if !n.contains(&y) {
                n.push(y);
            }
        }
    }
    let mut line = FxHashMap::default();
    for (&v, n) in &nb {
        if n.len() != 2 || is_input(v) {
            continue;
        }
        let p = positions[v as usize].map(f64::from);
        let (a, b) = (
            positions[n[0] as usize].map(f64::from),
            positions[n[1] as usize].map(f64::from),
        );
        let d1 = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
        let d2 = [b[0] - p[0], b[1] - p[1], b[2] - p[2]];
        let c = [
            d1[1] * d2[2] - d1[2] * d2[1],
            d1[2] * d2[0] - d1[0] * d2[2],
            d1[0] * d2[1] - d1[1] * d2[0],
        ];
        let dd = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
        let scale = dd(d1, d1).max(dd(d2, d2));
        if dd(c, c) <= 1e-10 * scale * scale && dd(d1, d2) > 0.0 {
            line.insert(v, (n[0], n[1]));
        }
    }
    line
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
    assemble_with(positions, tri_verts, face_ids, input_faces_num, None)
}

/// [`assemble`], and with `reroute` (`is_input` of an output vertex), the bridges merging left
/// at new in-line vertices are moved away first ([`reroute_bridges`]) so those vertices are
/// dissolved too (the EXACT stand-in: Blender's exact solver keeps no such vertices).
pub fn assemble_with(
    positions: Vec<[f32; 3]>,
    tri_verts: &[u32],
    face_ids: &[u32],
    input_faces_num: usize,
    reroute: Option<&dyn Fn(u32) -> bool>,
) -> Assembly {
    let tris_num = tri_verts.len() / 3;
    let mut groups: Vec<SmallVec<[u32; 4]>> = vec![SmallVec::new(); input_faces_num];
    for (t, &fid) in face_ids.iter().enumerate().take(tris_num) {
        frap_math::budget::check();
        if (fid as usize) < input_faces_num {
            groups[fid as usize].push(t as u32);
        }
    }
    let mut faces = Vec::with_capacity(tris_num);
    let mut ranges = Vec::new();
    for (gid, group) in groups.iter().enumerate() {
        frap_math::budget::check();
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
        if gf.len() > 1 {
            ranges.push(faces.len()..faces.len() + gf.len());
        }
        faces.extend(gf);
    }
    if let Some(is_input) = reroute {
        let line = in_line_vertices(&positions, &faces, is_input);
        let pos = |v: u32| positions[v as usize].map(f64::from);
        for r in ranges {
            reroute_bridges(&mut faces[r], &line, &pos, &|_, _| false);
        }
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
    fn bridge_moves_off_an_in_line_vertex() {
        // The plane at z = 0.25 cut by the default cube: its outer ring was left as two faces
        // bridged at 7, which lies on the cut line 5-7-9 (where the cube's diagonal crossed).
        let mut pos = [[0.0f64; 3]; 17];
        for (i, p) in [
            (3, [-1.0, -1.0]),
            (5, [-1.0, 1.0]),
            (7, [0.25, 1.0]),
            (9, [1.0, 1.0]),
            (12, [1.0, -1.0]),
            (13, [-1.5, -1.5]),
            (14, [1.5, -1.5]),
            (15, [1.5, 1.5]),
            (16, [-1.5, 1.5]),
        ] {
            pos[i] = [p[0], p[1], 0.25];
        }
        let mut group = vec![
            f(&[12, 3, 5, 7, 16, 13, 14, 15, 9]),
            f(&[5, 3, 12, 9, 7]),
            f(&[9, 15, 16, 7]),
        ];
        let line: FxHashMap<u32, (u32, u32)> = [(7, (5, 9))].into_iter().collect();
        reroute_bridges(&mut group, &line, &|v| pos[v as usize], &|_, _| false);
        for g in &group {
            if let Some(i) = g.find_vert_index(7) {
                let k = g.verts.len();
                let mut n = [g.verts[(i + k - 1) % k], g.verts[(i + 1) % k]];
                n.sort_unstable();
                assert_eq!(n, [5, 9], "{group:?}");
            }
        }
        assert_eq!(group[0].verts.as_slice(), &[12, 3, 5, 16, 13, 14, 15, 9]);
        assert_eq!(group[2].verts.as_slice(), &[9, 15, 16, 5, 7]);
        // A reflex corner is no ear: nothing moves.
        let mut group2 = vec![f(&[13, 5, 7, 16]), f(&[16, 7, 9])];
        pos[16] = [0.25, 2.0, 0.25];
        let before = group2.clone();
        reroute_bridges(&mut group2, &line, &|v| pos[v as usize], &|_, _| false);
        assert_eq!(group2, before);
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
