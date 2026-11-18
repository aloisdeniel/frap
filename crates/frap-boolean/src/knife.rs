// SPDX-License-Identifier: GPL-3.0-or-later
//! The knife (`mesh.intersect`, Blender's `BM_mesh_boolean_knife` / `BoolOpType::None`): the
//! faces of the operands are cut along their intersections and nothing is removed.
//!
//! Blender computes it with its exact mesh arrangement (`trimesh_nary_intersect`, not ported:
//! WP 8.6). frap uses manifold-rust's exact intersection graph
//! (`robust::intersection_graph::build_graph`, exact rational arithmetic): every input triangle
//! is subdivided along the segments where it meets triangles of the other operand. The pieces
//! of one input face are merged back into polygons except across the cut lines
//! (`crate::assemble::merge_out_faces_with`), and the new vertices left with exactly two
//! neighbours on a straight line (where a cut line crossed an internal triangulation edge)
//! are dissolved, as Blender's `polymesh_from_trimesh_with_dissolve` does. Vertices at the
//! same exact position are one vertex (Blender's vertex arena), so the cut lines are shared
//! by the faces on both sides ("Merge").
//!
//! Self intersection (`mode='SELECT'`) cuts every connected part of the operand with the
//! others; a part's intersections with itself are not found (T3 deviation).

use crate::{BoolError, BoolMesh, Src};

/// The cut faces.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct KnifeResult {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// The input vertex at exactly each output vertex's position (`None`: a cut vertex).
    pub vert_src: Vec<Option<Src>>,
    /// Face offsets (`faces + 1` entries).
    pub face_offsets: Vec<u32>,
    /// Vertex of each corner.
    pub corner_verts: Vec<u32>,
    /// Source face of each output face.
    pub face_src: Vec<Src>,
    /// The output edges on cut lines (vertex pairs, lower index first).
    pub isect_edges: Vec<[u32; 2]>,
    /// Whether any two operands' triangles intersect at all.
    pub any_intersections: bool,
}

#[cfg(feature = "manifold")]
pub use imp::knife;

#[cfg(not(feature = "manifold"))]
/// Not available without the `manifold` feature.
pub fn knife(operands: &[BoolMesh<'_>], self_intersect: bool) -> Result<KnifeResult, BoolError> {
    let _ = (operands, self_intersect);
    Err(BoolError::Unavailable(
        "frap: built without the boolean-manifold feature".into(),
    ))
}

#[cfg(feature = "manifold")]
mod imp {
    use manifold_rust::linalg::{Vec3, cross, dot};
    use manifold_rust::robust::intersection_graph::{IntersectionGraph, build_graph};
    use rustc_hash::{FxHashMap, FxHashSet};
    use smallvec::SmallVec;

    use super::*;
    use crate::assemble::{OutFace, merge_out_faces_with};
    use crate::manifold::imp::{Prepared, prepare};

    type Key = [u64; 3];

    fn key(v: Vec3) -> Key {
        // `+ 0.0` folds -0.0 into 0.0 so both spellings of zero are one point.
        [
            (v.x + 0.0).to_bits(),
            (v.y + 0.0).to_bits(),
            (v.z + 0.0).to_bits(),
        ]
    }

    fn v3(p: [f32; 3]) -> Vec3 {
        Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
    }

    /// The non-degenerate triangles of `tris` (as positions) and their indices.
    fn soup(p: &Prepared, tris: &[usize]) -> (Vec<[Vec3; 3]>, Vec<usize>) {
        let mut out = Vec::with_capacity(tris.len());
        let mut idx = Vec::with_capacity(tris.len());
        for &t in tris {
            let [a, b, c] = p.tris[t].map(|v| p.positions[v as usize]);
            let (a, b, c) = (v3(a), v3(b), v3(c));
            let n = cross(b - a, c - a);
            if n.x == 0.0 && n.y == 0.0 && n.z == 0.0 {
                continue;
            }
            out.push([a, b, c]);
            idx.push(t);
        }
        (out, idx)
    }

    /// Output accumulation across graphs.
    #[derive(Default)]
    struct Out {
        ids: FxHashMap<Key, u32>,
        positions: Vec<Vec3>,
        /// (operand, face) -> triangles of output vertex ids.
        groups: FxHashMap<(u32, u32), Vec<[u32; 3]>>,
        isect: FxHashSet<(u32, u32)>,
        any: bool,
    }

    impl Out {
        fn vert(&mut self, p: Vec3) -> u32 {
            let k = key(p);
            if let Some(&i) = self.ids.get(&k) {
                return i;
            }
            let i = self.positions.len() as u32;
            self.ids.insert(k, i);
            self.positions.push(p);
            i
        }

        /// Adds the pieces of graph `g` whose mesh is in `keep` (`(graph mesh, operand,
        /// triangle index map)`).
        fn add(&mut self, g: &IntersectionGraph, keep: &[(u8, u32, &[usize], &Prepared)]) {
            self.any |= g.any_intersections;
            let ids: Vec<u32> = g.verts_f64.iter().map(|&p| self.vert(p)).collect();
            for &(a, b) in &g.isect_edges {
                let (a, b) = (ids[a as usize], ids[b as usize]);
                if a != b {
                    self.isect.insert((a.min(b), a.max(b)));
                }
            }
            for piece in &g.pieces {
                let Some(&(_, op, map, prep)) = keep.iter().find(|k| k.0 == piece.mesh) else {
                    continue;
                };
                let t = map[piece.tri];
                let face = prep.face_of_tri[t];
                let mut tri = piece.vi.map(|v| ids[v as usize]);
                if piece.flipped {
                    tri.swap(1, 2);
                }
                if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
                    continue;
                }
                self.groups.entry((op, face)).or_default().push(tri);
            }
        }
    }

    /// Connected parts of an operand's faces (shared vertices), as triangle lists.
    fn components(p: &Prepared) -> Vec<Vec<usize>> {
        let n = p.positions.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        for t in &p.tris {
            let a = find(&mut parent, t[0] as usize);
            for &v in &t[1..] {
                let b = find(&mut parent, v as usize);
                if a != b {
                    parent[b] = a;
                }
            }
        }
        let mut comp_of: FxHashMap<usize, usize> = FxHashMap::default();
        let mut comps: Vec<Vec<usize>> = Vec::new();
        for (i, t) in p.tris.iter().enumerate() {
            let r = find(&mut parent, t[0] as usize);
            let c = *comp_of.entry(r).or_insert_with(|| {
                comps.push(Vec::new());
                comps.len() - 1
            });
            comps[c].push(i);
        }
        comps
    }

    /// Cuts the faces of `operands[0]` with those of `operands[1]`, or with
    /// `self_intersect`, the connected parts of `operands[0]` with each other.
    pub fn knife(
        operands: &[BoolMesh<'_>],
        self_intersect: bool,
    ) -> Result<KnifeResult, BoolError> {
        let prepared: Vec<Prepared> = operands.iter().map(prepare).collect();
        let mut out = Out::default();
        if self_intersect {
            let p = &prepared[0];
            let comps = components(p);
            if comps.len() < 2 {
                for (t, tri) in p.tris.iter().enumerate() {
                    let ids = tri.map(|v| out.vert(v3(p.positions[v as usize])));
                    out.groups
                        .entry((0, p.face_of_tri[t]))
                        .or_default()
                        .push(ids);
                }
            } else {
                for (ci, c) in comps.iter().enumerate() {
                    let rest: Vec<usize> = comps
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != ci)
                        .flat_map(|(_, c)| c.iter().copied())
                        .collect();
                    let (ps, pm) = soup(p, c);
                    let (qs, _) = soup(p, &rest);
                    let g = build_graph(&ps, &qs);
                    out.add(&g, &[(0, 0, &pm, p)]);
                }
            }
        } else {
            if operands.len() < 2 {
                return Err(BoolError::Input("knife: two operands needed".into()));
            }
            let all = |p: &Prepared| (0..p.tris.len()).collect::<Vec<usize>>();
            let (ps, pm) = soup(&prepared[0], &all(&prepared[0]));
            let (qs, qm) = soup(&prepared[1], &all(&prepared[1]));
            let g = build_graph(&ps, &qs);
            out.add(&g, &[(0, 0, &pm, &prepared[0]), (1, 1, &qm, &prepared[1])]);
        }
        Ok(finish(out, operands, &prepared))
    }

    /// Merges the pieces into polygons, dissolves the straight valence-2 cut vertices and
    /// builds the result.
    fn finish(out: Out, operands: &[BoolMesh<'_>], prepared: &[Prepared]) -> KnifeResult {
        // Input vertices by exact position (the first operand wins).
        let mut input: FxHashMap<Key, Src> = FxHashMap::default();
        for (o, p) in prepared.iter().enumerate() {
            for (i, &pos) in p.positions.iter().enumerate() {
                input.entry(key(v3(pos))).or_insert((o as u32, i as u32));
            }
        }
        let isect = &out.isect;
        let mut faces: Vec<OutFace> = Vec::new();
        let mut face_src: Vec<Src> = Vec::new();
        for (o, op) in operands.iter().enumerate() {
            for f in 0..op.faces_num() as u32 {
                let Some(tris) = out.groups.get(&(o as u32, f)) else {
                    continue;
                };
                let mut group: Vec<OutFace> = tris
                    .iter()
                    .map(|t| OutFace {
                        verts: SmallVec::from_slice(t),
                        face_id: f,
                    })
                    .collect();
                merge_out_faces_with(&mut group, &|a, b| !isect.contains(&(a.min(b), a.max(b))));
                for g in group {
                    faces.push(g);
                    face_src.push((o as u32, f));
                }
            }
        }
        let vert_src: Vec<Option<Src>> = out
            .positions
            .iter()
            .map(|&p| input.get(&key(p)).copied())
            .collect();
        // Dissolve the cut vertices with exactly two neighbours on a straight line.
        let n = out.positions.len();
        let mut nbrs: Vec<SmallVec<[u32; 4]>> = vec![SmallVec::new(); n];
        for f in &faces {
            let k = f.verts.len();
            for i in 0..k {
                let (a, b) = (f.verts[i], f.verts[(i + 1) % k]);
                if !nbrs[a as usize].contains(&b) {
                    nbrs[a as usize].push(b);
                }
                if !nbrs[b as usize].contains(&a) {
                    nbrs[b as usize].push(a);
                }
            }
        }
        let mut dissolve = vec![false; n];
        for v in 0..n {
            if vert_src[v].is_none() && nbrs[v].len() == 2 {
                let (a, b) = (
                    out.positions[nbrs[v][0] as usize],
                    out.positions[nbrs[v][1] as usize],
                );
                let p = out.positions[v];
                let (d1, d2) = (p - a, b - p);
                let c = cross(d1, d2);
                let scale = dot(d1, d1).max(dot(d2, d2));
                dissolve[v] = dot(c, c) <= 1e-12 * scale * scale && dot(d1, d2) > 0.0;
            }
        }
        for f in &faces {
            let left = f.verts.iter().filter(|&&v| !dissolve[v as usize]).count();
            if left < 3 {
                for &v in &f.verts {
                    dissolve[v as usize] = false;
                }
            }
        }
        let mut remap = vec![u32::MAX; n];
        let mut r = KnifeResult {
            face_offsets: vec![0],
            any_intersections: out.any,
            ..Default::default()
        };
        for v in 0..n {
            if !dissolve[v] {
                remap[v] = r.positions.len() as u32;
                let p = out.positions[v];
                r.positions.push([p.x as f32, p.y as f32, p.z as f32]);
                r.vert_src.push(vert_src[v]);
            }
        }
        let mut edges: Vec<[u32; 2]> = Vec::new();
        for (f, src) in faces.iter().zip(face_src) {
            let k = f.verts.len();
            let start = r.corner_verts.len();
            // An output edge is on a cut line when its first piece (before the dissolved
            // vertices it spans) is.
            let mut cut: Vec<bool> = Vec::new();
            for i in 0..k {
                let v = f.verts[i];
                if remap[v as usize] == u32::MAX {
                    continue;
                }
                let next = f.verts[(i + 1) % k];
                r.corner_verts.push(remap[v as usize]);
                cut.push(isect.contains(&(v.min(next), v.max(next))));
            }
            let fv = &r.corner_verts[start..];
            for (i, &c) in cut.iter().enumerate() {
                if c {
                    let (a, b) = (fv[i], fv[(i + 1) % fv.len()]);
                    edges.push([a.min(b), a.max(b)]);
                }
            }
            r.face_offsets.push(r.corner_verts.len() as u32);
            r.face_src.push(src);
        }
        edges.sort_unstable();
        edges.dedup();
        r.isect_edges = edges;
        r
    }
}

#[cfg(all(test, feature = "manifold"))]
mod tests {
    use super::*;

    #[test]
    fn plane_cuts_cube_faces() {
        // Blender's default cube and a horizontal plane through it at z = 0.25.
        let p: Vec<[f32; 3]> = vec![
            [-1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
        ];
        let fo = vec![0, 4, 8, 12, 16, 20, 24];
        let cv = vec![
            0, 1, 3, 2, 2, 3, 7, 6, 6, 7, 5, 4, 4, 5, 1, 0, 2, 6, 4, 0, 7, 3, 1, 5,
        ];
        let q: Vec<[f32; 3]> = vec![
            [-2.0, -2.0, 0.25],
            [2.0, -2.0, 0.25],
            [2.0, 2.0, 0.25],
            [-2.0, 2.0, 0.25],
        ];
        let qfo = vec![0, 4];
        let qcv = vec![0, 1, 2, 3];
        let r = knife(
            &[BoolMesh::new(&p, &fo, &cv), BoolMesh::new(&q, &qfo, &qcv)],
            false,
        )
        .unwrap();
        assert!(r.any_intersections);
        // The four side faces are cut in two; top and bottom stay.
        let cube_faces = r.face_src.iter().filter(|s| s.0 == 0).count();
        assert_eq!(cube_faces, 10);
        // The cut lines run around the cube at z = 0.25 (a vertex where a plane triangle edge
        // left a bridge to the plane's outer ring may split one of them).
        assert!(
            (4..=8).contains(&r.isect_edges.len()),
            "{:?}",
            r.isect_edges
        );
        for e in &r.isect_edges {
            for &v in e {
                let p = r.positions[v as usize];
                assert_eq!(p[2], 0.25);
                assert!(p[0].abs() == 1.0 || p[1].abs() == 1.0, "{p:?}");
            }
        }
        // The corners of the cut square are the new vertices on the cube's vertical edges.
        let corners = r
            .positions
            .iter()
            .zip(&r.vert_src)
            .filter(|(p, s)| s.is_none() && p[0].abs() == 1.0 && p[1].abs() == 1.0)
            .count();
        assert_eq!(corners, 4);
        // Every face is a proper polygon.
        assert!(r.face_offsets.windows(2).all(|w| w[1] - w[0] >= 3));
    }

    #[test]
    fn self_intersection_cuts_parts() {
        // Two crossing quads (connected parts of one operand).
        let p: Vec<[f32; 3]> = vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [0.0, -2.0, -1.0],
            [0.0, 2.0, -1.0],
            [0.0, 2.0, 1.0],
            [0.0, -2.0, 1.0],
        ];
        let fo = vec![0, 4, 8];
        let cv = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let r = knife(&[BoolMesh::new(&p, &fo, &cv)], true).unwrap();
        assert!(r.any_intersections);
        // The horizontal quad is cut in two along x = 0; the vertical one keeps one face
        // with the cut line inside (an open cut has no polygon to split).
        assert_eq!(r.face_src.iter().filter(|s| s.1 == 0).count(), 2);
        assert!(r.isect_edges.iter().all(|e| {
            e.iter()
                .all(|&v| r.positions[v as usize][0] == 0.0 && r.positions[v as usize][2] == 0.0)
        }));
    }
}
