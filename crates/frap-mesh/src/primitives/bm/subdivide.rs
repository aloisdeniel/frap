// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.subdivide_edges` restricted to what `create_icosphere` calls: every edge of an
//! all-triangle mesh selected, `cuts` uniform cuts, `use_grid_fill` (pattern `tri_3edge`) and
//! `use_sphere` smoothing. Ported from blender/source/blender/bmesh/operators/bmo_subdivide.cc
//! (`bmo_subdivide_edges_exec`, `bm_subdivide_multicut`, `subdivide_edge_num`, `alter_co`,
//! `tri_3edge_subdivide`, `connect_smallest_face`) with the kernel operations it relies on from
//! bmesh_core.cc / bmesh_mods.cc (`BM_edge_split` / split-edge-make-vert: the new vertex and edge
//! are appended, the new loop follows the split loop, UVs interpolated by
//! `BM_data_interp_face_vert_edge`; `BM_face_split` / split-face-make-edge: new edge, new face
//! appended, `l_first` rules). No element is killed, so creation order is the final order.

use super::math::normalize_v3_length;
use super::{Bm, Face, V3};

struct Subd<'a> {
    bm: &'a mut Bm,
    /// Faces using each vertex slot (ascending face ids are not required).
    vert_faces: Vec<Vec<u32>>,
    /// The `CD_SHAPEKEY` temp layer: displaced coordinates, copied to `co` at the end.
    tmp: Vec<V3>,
    smooth: f32,
}

impl Subd<'_> {
    fn new_vert(&mut self, co: V3) -> u32 {
        let v = self.bm.vert_create(co);
        if self.vert_faces.len() <= v as usize {
            self.vert_faces.resize(v as usize + 1, Vec::new());
            self.tmp.resize(v as usize + 1, [0.0; 3]);
        }
        self.tmp[v as usize] = co;
        v
    }

    /// `alter_co` with `use_sphere`.
    fn alter_co(&mut self, v: u32) {
        let co = self.bm.co(v);
        self.tmp[v as usize] = normalize_v3_length(co, self.smooth);
    }

    /// `BM_edge_split(bm, e, e->v1, &e_new, fac)`: returns the new vertex.
    fn edge_split(&mut self, e: u32, fac: f32) -> u32 {
        let [tv, v_other] = *self.bm.edges.get(e).expect("live edge");
        let co_v = self.bm.co(tv);
        let co_o = self.bm.co(v_other);
        // SEMV: v_new created at tv->co, e_new = (tv, v_new), e becomes (v_new, v_other).
        let d = [co_o[0] - co_v[0], co_o[1] - co_v[1], co_o[2] - co_v[2]];
        let co = [
            co_v[0] + d[0] * fac,
            co_v[1] + d[1] * fac,
            co_v[2] + d[2] * fac,
        ];
        let v_new = self.new_vert(co);
        self.bm.edge_create_always(tv, v_new);
        self.bm.edge_set_verts(e, v_new, v_other);
        // Insert a loop between tv and v_other in every face using the edge.
        let faces: Vec<u32> = self.vert_faces[tv as usize].clone();
        for fi in faces {
            let f = self.bm.faces.get_mut(fi).expect("live face");
            let n = f.verts.len();
            let pos = (0..n).find(|&k| {
                let (a, b) = (f.verts[k], f.verts[(k + 1) % n]);
                (a == tv && b == v_other) || (a == v_other && b == tv)
            });
            let Some(k) = pos else { continue };
            let (uv_a, uv_b) = (f.uvs[k], f.uvs[(k + 1) % n]);
            let (uv_tv, uv_o) = if f.verts[k] == tv {
                (uv_a, uv_b)
            } else {
                (uv_b, uv_a)
            };
            // CustomData interpolation: result = 0 + uv_other * fac + uv_tv * (1 - fac).
            let w = 1.0 - fac;
            let mut uv = [0.0f32; 2];
            uv = [uv[0] + uv_o[0] * fac, uv[1] + uv_o[1] * fac];
            uv = [uv[0] + uv_tv[0] * w, uv[1] + uv_tv[1] * w];
            f.verts.insert(k + 1, v_new);
            f.uvs.insert(k + 1, uv);
            self.vert_faces[v_new as usize].push(fi);
        }
        v_new
    }

    /// `subdivide_edge_num` + `alter_co` for cut `cur` of `tot` on edge `e`.
    fn edge_cut(&mut self, e: u32, cur: usize, tot: usize) -> u32 {
        let fac = 1.0f32 / (tot + 1 - cur) as f32;
        let v = self.edge_split(e, fac);
        self.alter_co(v);
        v
    }

    /// `BM_vert_pair_share_face_by_len(v_a, v_b, …, allow_adjacent=true)`: the smallest face
    /// using both vertices.
    fn share_face(&self, v_a: u32, v_b: u32) -> Option<u32> {
        let mut best: Option<(usize, u32)> = None;
        for &f in &self.vert_faces[v_a as usize] {
            let face = self.bm.faces.get(f).expect("live face");
            if face.verts.contains(&v_b) && best.is_none_or(|(len, _)| face.verts.len() < len) {
                best = Some((face.verts.len(), f));
            }
        }
        best.map(|(_, f)| f)
    }

    /// `connect_smallest_face(v_a, v_b)` → `BM_face_split(f, l_a, l_b)`: returns the new edge.
    fn connect(&mut self, v_a: u32, v_b: u32) -> u32 {
        let fi = self.share_face(v_a, v_b).expect("vertices share a face");
        let f = self.bm.faces.get(fi).expect("live face").clone();
        let n = f.verts.len();
        let ia = f.verts.iter().position(|&v| v == v_a).expect("v_a in face");
        let ib = f.verts.iter().position(|&v| v == v_b).expect("v_b in face");
        let e = self.bm.edge_create_always(v_a, v_b);
        // f keeps l_v1 .. l_v2.prev + l_f1(v_b); f2 gets l_v2 .. l_v1.prev + l_f2(v_a).
        let ring = |from: usize, to: usize| -> Vec<usize> {
            let mut out = Vec::new();
            let mut k = from;
            while k != to {
                out.push(k);
                k = (k + 1) % n;
            }
            out
        };
        let r1 = ring(ia, ib);
        let r2 = ring(ib, ia);
        // Loops: Some(old index) or None for the new loop (last in each ring).
        let mut f1: Vec<Option<usize>> = r1.iter().map(|&k| Some(k)).collect();
        f1.push(None);
        let mut f2: Vec<Option<usize>> = r2.iter().map(|&k| Some(k)).collect();
        f2.push(None);
        let rot = |ringv: &mut Vec<Option<usize>>, first: Option<usize>| {
            let p = ringv
                .iter()
                .position(|&x| x == first)
                .expect("first loop in ring");
            ringv.rotate_left(p);
        };
        let first_in_f1 = r1.contains(&0);
        if first_in_f1 {
            rot(&mut f1, Some(0));
            // f2: prev of l_f2 if old first was l_v1; l_v2 if old first was l_v2.prev; else l_f2.
            let f2_first = if ia == 0 {
                Some((ia + n - 1) % n)
            } else if (ib + n - 1).is_multiple_of(n) {
                Some(ib)
            } else {
                None
            };
            rot(&mut f2, f2_first);
        } else {
            rot(&mut f2, Some(0));
            let f1_first = if ib == 0 {
                Some((ib + n - 1) % n)
            } else if (ia + n - 1).is_multiple_of(n) {
                Some(ia)
            } else {
                None
            };
            rot(&mut f1, f1_first);
        }
        let build = |ringv: &[Option<usize>], new_v: u32, new_uv_from: usize| -> Face {
            let mut face = Face::default();
            for x in ringv {
                match x {
                    Some(k) => {
                        face.verts.push(f.verts[*k]);
                        face.uvs.push(f.uvs[*k]);
                    }
                    None => {
                        face.verts.push(new_v);
                        face.uvs.push(f.uvs[new_uv_from]);
                    }
                }
            }
            face
        };
        let face1 = build(&f1, v_b, ib);
        let face2 = build(&f2, v_a, ia);
        // Adjacency: vertices only in f2 move to the new face.
        let f2_id = self.bm.faces.alloc(face2.clone());
        for &v in &face2.verts {
            let list = &mut self.vert_faces[v as usize];
            if v != v_a && v != v_b {
                list.retain(|&x| x != fi);
            }
            list.push(f2_id);
        }
        *self.bm.faces.get_mut(fi).expect("live face") = face1;
        e
    }

    /// `tri_3edge_subdivide(bm, face, verts, params)`.
    fn tri_3edge(&mut self, verts: &[u32], numcuts: usize) {
        let mut lines: Vec<Vec<u32>> = vec![Vec::new(); numcuts + 2];
        lines[0] = vec![verts[numcuts * 2 + 1]];
        let mut last = vec![verts[numcuts * 3 + 2]];
        last.extend_from_slice(&verts[..numcuts]);
        last.push(verts[numcuts]);
        lines[numcuts + 1] = last;
        for i in 0..numcuts {
            let a = numcuts * 2 + 2 + i;
            let b = numcuts + numcuts - i;
            let e = self.connect(verts[a], verts[b]);
            let mut line = vec![verts[a]];
            for j in 0..i {
                line.push(self.edge_cut(e, j, i));
            }
            line.push(verts[b]);
            lines[i + 1] = line;
        }
        for i in 1..=numcuts {
            for j in 0..i {
                self.connect(lines[i][j], lines[i + 1][j + 1]);
                self.connect(lines[i][j + 1], lines[i + 1][j + 1]);
            }
        }
    }
}

/// `subdivide_edges edges=<all> smooth=<smooth> cuts=<cuts> use_grid_fill=true use_sphere=true`
/// on a mesh of triangles.
pub(crate) fn subdivide_sphere_tris(bm: &mut Bm, cuts: usize, smooth: f32) {
    if cuts == 0 {
        return;
    }
    let nv = bm.verts.slots.len();
    let mut vert_faces = vec![Vec::new(); nv];
    for (fi, f) in bm.faces.iter() {
        for &v in &f.verts {
            vert_faces[v as usize].push(fi);
        }
    }
    let tmp: Vec<V3> = bm
        .verts
        .slots
        .iter()
        .map(|v| v.unwrap_or([0.0; 3]))
        .collect();
    let mut s = Subd {
        bm,
        vert_faces,
        tmp,
        smooth,
    };
    // Pattern matching: every face is a fully selected triangle → tri_3edge, start = l_first.
    let facedata: Vec<(u32, u32)> = s.bm.faces.iter().map(|(i, f)| (i, f.verts[0])).collect();
    // Split edges in edge order.
    let einput: Vec<u32> = s.bm.edges.iter().map(|(i, _)| i).collect();
    for e in einput {
        let [v1, v2] = *s.bm.edges.get(e).expect("live edge");
        for i in 0..cuts {
            s.edge_cut(e, i, cuts);
        }
        s.alter_co(v1);
        s.alter_co(v2);
    }
    for (i, slot) in s.bm.verts.slots.iter_mut().enumerate() {
        if let Some(co) = slot {
            *co = s.tmp[i];
        }
    }
    // Patterns, from the facedata stack (last face first).
    for &(fi, start) in facedata.iter().rev() {
        let f = s.bm.faces.get(fi).expect("live face");
        let n = f.verts.len();
        let j = f
            .verts
            .iter()
            .position(|&v| v == start)
            .expect("start vertex");
        let a = j + 1;
        let mut verts = vec![0u32; n];
        for (k, &v) in f.verts.iter().enumerate() {
            verts[(k + n - a % n) % n] = v;
        }
        s.tri_3edge(&verts, cuts);
    }
    for (i, slot) in s.bm.verts.slots.iter_mut().enumerate() {
        if let Some(co) = slot {
            *co = s.tmp[i];
        }
    }
}
