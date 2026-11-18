// SPDX-License-Identifier: GPL-3.0-or-later
//! The element layout of Blender's subdivided mesh (`subdiv_foreach.cc`): which vertex, edge,
//! face and corner indices the result has, and for each vertex and corner the ptex face and
//! `(u, v)` it is evaluated at. Following it makes SUBSURF index-exact (T1) with Blender:
//!
//! - vertices: one per coarse vertex (same index), then `resolution - 2` per coarse edge, then
//!   the inner vertices of every coarse face (a `(resolution - 2)²` grid for a quad; the centre
//!   and one grid per corner for other faces);
//! - edges: `resolution - 1` per coarse edge, then the inner edges of every face;
//! - faces and corners: per coarse face, inner quads first, then the ring along the boundary.
//!
//! A quad has one ptex face spanning it (`u` from corner 0 to 1, `v` from corner 0 to 3); any
//! other face has one ptex face per corner (corner, next edge's middle, centre, previous
//! edge's middle) at half the resolution. Coarse vertices and edge vertices are evaluated
//! through the first face that uses them, as Blender's single-threaded pass does.
//!
//! Ported from blender/source/blender/blenkernel/intern/subdiv_foreach.cc (Blender 5.2.4).
// The index arithmetic (running counters next to loop indices) is kept as in the C++ so the
// port can be checked line by line.
#![allow(clippy::explicit_counter_loop)]

/// Where a result vertex's position comes from.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PosSource {
    /// The limit (or refined) surface at a ptex face coordinate.
    Ptex {
        /// Ptex face index.
        ptex: u32,
        /// `u`
        u: f32,
        /// `v`
        v: f32,
    },
    /// The coarse vertex itself (a loose vertex).
    Coarse(u32),
    /// A point of a loose edge (`mesh_interpolate_position_on_edge`).
    LooseEdge {
        /// Coarse edge.
        edge: u32,
        /// Parameter along the edge.
        u: f32,
    },
}

/// Where a result vertex's (or corner's) other attributes come from (`subdiv_mesh.cc`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DataSource {
    /// Copied from a coarse element (vertex for vertex data).
    Copy(u32),
    /// Bilinear in the ptex face of coarse face `face` at its corner `corner` (0 for quads):
    /// for a quad the four face corners, otherwise (corner, next edge middle, face centre,
    /// previous edge middle) (`VerticesForInterpolation` / `LoopsForInterpolation`).
    Ptex {
        /// Coarse face.
        face: u32,
        /// Ptex corner within the face (0 for quads).
        corner: u32,
        /// `u`
        u: f32,
        /// `v`
        v: f32,
    },
    /// `mix2(u, edge.v0, edge.v1)` of a loose edge's interior point.
    Edge {
        /// Coarse edge.
        edge: u32,
        /// Factor.
        u: f32,
    },
    /// Unchanged (loose-edge end points keep the data their coarse vertex gave them).
    Keep,
}

/// The result layout.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// Grid resolution (`(1 << level) + 1`).
    pub resolution: u32,
    /// First ptex face of each coarse face (`faces + 1` entries).
    pub face_ptex_offset: Vec<u32>,
    /// Per result vertex: position source, in write order (a later write wins).
    pub vert_pos: Vec<PosSource>,
    /// Per result vertex: attribute source.
    pub vert_data: Vec<DataSource>,
    /// Result edges.
    pub edges: Vec<[u32; 2]>,
    /// Coarse edge of each result edge (`None` for inner edges).
    pub edge_origin: Vec<Option<u32>>,
    /// Corner vertices (4 per face).
    pub corner_verts: Vec<u32>,
    /// Corner edges.
    pub corner_edges: Vec<u32>,
    /// Per corner: ptex face and coordinate.
    pub corner_ptex: Vec<(u32, f32, f32)>,
    /// Per corner: attribute source (`DataSource::Ptex`).
    pub corner_data: Vec<DataSource>,
    /// Coarse face of each result face.
    pub face_origin: Vec<u32>,
}

/// The coarse mesh the layout is computed for.
#[derive(Copy, Clone, Debug)]
pub struct Coarse<'a> {
    /// Vertex count.
    pub nverts: usize,
    /// Edges.
    pub edges: &'a [[u32; 2]],
    /// Face offsets (`faces + 1`).
    pub face_offsets: &'a [u32],
    /// Corner vertices.
    pub corner_verts: &'a [u32],
    /// Corner edges.
    pub corner_edges: &'a [u32],
}

impl Coarse<'_> {
    fn nfaces(&self) -> usize {
        self.face_offsets.len().saturating_sub(1)
    }
    fn face(&self, f: usize) -> std::ops::Range<usize> {
        self.face_offsets[f] as usize..self.face_offsets[f + 1] as usize
    }
}

fn num_ptex_faces_per_face(size: usize) -> usize {
    if size == 4 { 1 } else { size }
}

fn num_edges_per_ptex_face(resolution: i64) -> i64 {
    2 * (resolution - 1) * resolution
}

fn num_inner_edges_per_ptex_face(resolution: i64) -> i64 {
    if resolution < 2 {
        return 0;
    }
    (resolution - 2) * resolution + (resolution - 1) * (resolution - 1)
}

fn num_faces_per_ptex(resolution: i64) -> i64 {
    (resolution - 1) * (resolution - 1)
}

fn ptex_face_resolution(size: usize, resolution: i64) -> i64 {
    if size == 4 {
        resolution
    } else {
        (resolution >> 1) + 1
    }
}

struct Ctx<'a> {
    c: Coarse<'a>,
    resolution: i64,
    vertices_corner_offset: i64,
    vertices_edge_offset: i64,
    vertices_inner_offset: i64,
    edge_boundary_offset: i64,
    edge_inner_offset: i64,
    subdiv_vert_offset: Vec<i64>,
    subdiv_edge_offset: Vec<i64>,
    subdiv_face_offset: Vec<i64>,
    face_ptex_offset: Vec<u32>,
    verts_used: Vec<bool>,
    edges_used: Vec<bool>,
    out: Layout,
}

impl Ctx<'_> {
    fn cv(&self, corner: usize) -> i64 {
        self.c.corner_verts[corner] as i64
    }
    fn ce(&self, corner: usize) -> i64 {
        self.c.corner_edges[corner] as i64
    }
    fn edge(&self, e: i64) -> [i64; 2] {
        let e = self.c.edges[e as usize];
        [e[0] as i64, e[1] as i64]
    }

    fn vert(&mut self, i: i64, pos: PosSource, data: DataSource) {
        let i = i as usize;
        self.out.vert_pos[i] = pos;
        if data != DataSource::Keep {
            self.out.vert_data[i] = data;
        }
    }

    fn set_edge(&mut self, i: i64, origin: Option<u32>, v1: i64, v2: i64) {
        let i = i as usize;
        self.out.edges[i] = [v1 as u32, v2 as u32];
        self.out.edge_origin[i] = origin;
    }

    fn coarse_edge_origin(e: i64) -> Option<u32> {
        if e < 0 { None } else { Some(e as u32) }
    }

    // ---- vertices ------------------------------------------------------------------------

    fn corner_vertices_regular(&mut self, f: usize) {
        const W: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let face = self.c.face(f);
        let ptex = self.face_ptex_offset[f];
        for (corner, w) in W.iter().enumerate().take(face.len()) {
            let v = self.cv(face.start + corner);
            if std::mem::replace(&mut self.verts_used[v as usize], true) {
                continue;
            }
            let i = self.vertices_corner_offset + v;
            self.vert(
                i,
                PosSource::Ptex {
                    ptex,
                    u: w[0],
                    v: w[1],
                },
                DataSource::Copy(v as u32),
            );
        }
    }

    fn corner_vertices_special(&mut self, f: usize) {
        let face = self.c.face(f);
        let mut ptex = self.face_ptex_offset[f];
        for corner in 0..face.len() {
            let v = self.cv(face.start + corner);
            if !std::mem::replace(&mut self.verts_used[v as usize], true) {
                let i = self.vertices_corner_offset + v;
                self.vert(
                    i,
                    PosSource::Ptex {
                        ptex,
                        u: 0.0,
                        v: 0.0,
                    },
                    DataSource::Copy(v as u32),
                );
            }
            ptex += 1;
        }
    }

    fn edge_vertices_regular(&mut self, f: usize) {
        let face = self.c.face(f);
        let resolution_1 = self.resolution - 1;
        let inv = 1.0 / resolution_1 as f32;
        let per_edge = self.resolution - 2;
        let ptex = self.face_ptex_offset[f];
        for corner in 0..face.len() {
            let cvert = self.cv(face.start + corner);
            let e = self.ce(face.start + corner);
            if std::mem::replace(&mut self.edges_used[e as usize], true) {
                continue;
            }
            let ce = self.edge(e);
            let flip = ce[1] == cvert;
            let mut i = self.vertices_edge_offset + e * per_edge;
            for vi in 0..per_edge {
                let mut fac = (vi + 1) as f32 * inv;
                if flip {
                    fac = 1.0 - fac;
                }
                if corner >= 2 {
                    fac = 1.0 - fac;
                }
                let (u, v) = if corner & 1 == 0 {
                    (fac, if corner == 2 { 1.0 } else { 0.0 })
                } else {
                    (if corner == 1 { 1.0 } else { 0.0 }, fac)
                };
                self.vert(
                    i,
                    PosSource::Ptex { ptex, u, v },
                    DataSource::Ptex {
                        face: f as u32,
                        corner: 0,
                        u,
                        v,
                    },
                );
                i += 1;
            }
        }
    }

    fn edge_vertices_special(&mut self, f: usize) {
        let face = self.c.face(f);
        let n = face.len();
        let per_edge = self.resolution - 2;
        let per_ptex_edge = (self.resolution >> 1) + 1;
        let inv = 1.0 / (per_ptex_edge - 1) as f32;
        let ptex_start = self.face_ptex_offset[f];
        for corner in 0..n {
            let ptex = ptex_start + corner as u32;
            let cvert = self.cv(face.start + corner);
            let e = self.ce(face.start + corner);
            if std::mem::replace(&mut self.edges_used[e as usize], true) {
                continue;
            }
            let ce = self.edge(e);
            let flip = ce[1] == cvert;
            let mut i = self.vertices_edge_offset + e * per_edge;
            let mut delta = 1;
            if flip {
                i += per_edge - 1;
                delta = -1;
            }
            for vi in 1..per_ptex_edge {
                let u = vi as f32 * inv;
                self.vert(
                    i,
                    PosSource::Ptex { ptex, u, v: 0.0 },
                    DataSource::Ptex {
                        face: f as u32,
                        corner: corner as u32,
                        u,
                        v: 0.0,
                    },
                );
                i += delta;
            }
            let next = (corner + 1) % n;
            let next_ptex = ptex_start + next as u32;
            for vi in 1..per_ptex_edge - 1 {
                let v = 1.0 - vi as f32 * inv;
                self.vert(
                    i,
                    PosSource::Ptex {
                        ptex: next_ptex,
                        u: 0.0,
                        v,
                    },
                    DataSource::Ptex {
                        face: f as u32,
                        corner: next as u32,
                        u: 0.0,
                        v,
                    },
                );
                i += delta;
            }
        }
    }

    fn inner_vertices_regular(&mut self, f: usize) {
        let inv = 1.0 / (self.resolution - 1) as f32;
        let ptex = self.face_ptex_offset[f];
        let mut i = self.vertices_inner_offset + self.subdiv_vert_offset[f];
        for y in 1..self.resolution - 1 {
            let v = y as f32 * inv;
            for x in 1..self.resolution - 1 {
                let u = x as f32 * inv;
                self.vert(
                    i,
                    PosSource::Ptex { ptex, u, v },
                    DataSource::Ptex {
                        face: f as u32,
                        corner: 0,
                        u,
                        v,
                    },
                );
                i += 1;
            }
        }
    }

    fn inner_vertices_special(&mut self, f: usize) {
        let face = self.c.face(f);
        let pr = ptex_face_resolution(face.len(), self.resolution);
        let inv = 1.0 / (pr - 1) as f32;
        let mut ptex = self.face_ptex_offset[f];
        let mut i = self.vertices_inner_offset + self.subdiv_vert_offset[f];
        self.vert(
            i,
            PosSource::Ptex {
                ptex,
                u: 1.0,
                v: 1.0,
            },
            DataSource::Ptex {
                face: f as u32,
                corner: 0,
                u: 1.0,
                v: 1.0,
            },
        );
        i += 1;
        for corner in 0..face.len() {
            for y in 1..pr - 1 {
                let v = y as f32 * inv;
                for x in 1..pr {
                    let u = x as f32 * inv;
                    self.vert(
                        i,
                        PosSource::Ptex { ptex, u, v },
                        DataSource::Ptex {
                            face: f as u32,
                            corner: corner as u32,
                            u,
                            v,
                        },
                    );
                    i += 1;
                }
            }
            ptex += 1;
        }
    }

    // ---- edges ---------------------------------------------------------------------------

    fn edges_row(&mut self, start_edge: i64, start_vert: i64, per_row: i64) -> i64 {
        let mut e = start_edge;
        let mut v = start_vert;
        for _ in 0..per_row - 1 {
            self.set_edge(e, None, v, v + 1);
            e += 1;
            v += 1;
        }
        e
    }

    fn edges_column(&mut self, start_edge: i64, start_vert: i64, per_row: i64) -> i64 {
        let mut e = start_edge;
        let mut v = start_vert;
        for _ in 0..per_row {
            self.set_edge(e, None, v, v + per_row);
            e += 1;
            v += 1;
        }
        e
    }

    fn edges_regular(&mut self, f: usize) {
        let face = self.c.face(f);
        let res = self.resolution;
        let start_vert = self.vertices_inner_offset + self.subdiv_vert_offset[f];
        let per_edge = res - 2;
        let mut e = self.edge_inner_offset + self.subdiv_edge_offset[f];
        e = self.edges_row(e, start_vert, res - 2);
        for row in 0..res - 3 {
            let row_start = start_vert + row * (res - 2);
            e = self.edges_column(e, row_start, res - 2);
            e = self.edges_row(e, row_start + res - 2, res - 2);
        }
        for corner in 0..face.len() {
            let cvert = self.cv(face.start + corner);
            let ce_i = self.ce(face.start + corner);
            let ce = self.edge(ce_i);
            let start_edge_vert = self.vertices_edge_offset + ce_i * per_edge;
            let flip = ce[1] == cvert;
            let mut side_start = start_vert;
            let mut side_stride = 0;
            match corner {
                0 => side_stride = 1,
                1 => {
                    side_start += res - 3;
                    side_stride = res - 2;
                }
                2 => {
                    side_start += per_edge * per_edge - 1;
                    side_stride = -1;
                }
                3 => {
                    side_start += per_edge * (per_edge - 1);
                    side_stride = -(res - 2);
                }
                _ => {}
            }
            for i in 0..res - 2 {
                let v1 = if flip {
                    start_edge_vert + (res - i - 3)
                } else {
                    start_edge_vert + i
                };
                let v2 = side_start + side_stride * i;
                self.set_edge(e, None, v1, v2);
                e += 1;
            }
        }
    }

    fn edges_special(&mut self, f: usize) {
        let res = self.resolution;
        let face = self.c.face(f);
        let n = face.len() as i64;
        let pr = ptex_face_resolution(face.len(), res);
        let pir = pr - 2;
        let inner_verts_per_ptex = (pr - 1) * (pr - 2);
        let per_edge = res - 2;
        let center = self.vertices_inner_offset + self.subdiv_vert_offset[f];
        let start_vert = center + 1;
        let mut e = self.edge_inner_offset + self.subdiv_edge_offset[f];
        for corner in 0..n {
            let ptex_vert = start_vert + corner * inner_verts_per_ptex;
            e = self.edges_row(e, ptex_vert, pir + 1);
            for row in 0..pir - 1 {
                let row_start = ptex_vert + row * (pir + 1);
                e = self.edges_column(e, row_start, pir + 1);
                e = self.edges_row(e, row_start + pir + 1, pir + 1);
            }
        }
        for corner in 0..n {
            let next = (corner + 1) % n;
            let mut cur = start_vert + corner * inner_verts_per_ptex + pir;
            let mut nxt = start_vert + next * inner_verts_per_ptex + inner_verts_per_ptex - pr + 1;
            for _ in 0..pir {
                self.set_edge(e, None, cur, nxt);
                e += 1;
                cur += pir + 1;
                nxt += 1;
            }
        }
        if pr >= 3 {
            for corner in 0..n {
                let end = start_vert + corner * inner_verts_per_ptex + inner_verts_per_ptex - 1;
                self.set_edge(e, None, center, end);
                e += 1;
            }
        }
        let mut prev = n - 1;
        for corner in 0..n {
            let cvert = self.cv(face.start + corner as usize);
            let ce_i = self.ce(face.start + corner as usize);
            let prev_e = self.ce(face.start + prev as usize);
            {
                let ce = self.edge(ce_i);
                let start_edge_vert = self.vertices_edge_offset + ce_i * per_edge;
                let flip = ce[1] == cvert;
                let side_start = if pr >= 3 {
                    start_vert + inner_verts_per_ptex * corner
                } else {
                    center
                };
                for i in 0..pr - 1 {
                    let v1 = if flip {
                        start_edge_vert + (res - i - 3)
                    } else {
                        start_edge_vert + i
                    };
                    self.set_edge(e, None, v1, side_start + i);
                    e += 1;
                }
            }
            if pr >= 3 {
                let ce = self.edge(prev_e);
                let start_edge_vert = self.vertices_edge_offset + prev_e * per_edge;
                let flip = ce[1] == cvert;
                let side_start = start_vert + inner_verts_per_ptex * corner;
                for i in 0..pr - 2 {
                    let v1 = if flip {
                        start_edge_vert + (res - i - 3)
                    } else {
                        start_edge_vert + i
                    };
                    self.set_edge(e, None, v1, side_start + (pir + 1) * i);
                    e += 1;
                }
            }
            prev = corner;
        }
    }

    fn boundary_edges(&mut self, ce_i: i64) {
        let ce = self.edge(ce_i);
        let res = self.resolution;
        let per_edge_v = res - 2;
        let per_edge_e = res - 1;
        let mut e = self.edge_boundary_offset + ce_i * per_edge_e;
        let mut last = self.vertices_corner_offset + ce[0];
        let origin = Self::coarse_edge_origin(ce_i);
        for i in 0..per_edge_e - 1 {
            let v2 = self.vertices_edge_offset + ce_i * per_edge_v + i;
            self.set_edge(e, origin, last, v2);
            last = v2;
            e += 1;
        }
        let v2 = self.vertices_corner_offset + ce[1];
        self.set_edge(e, origin, last, v2);
    }

    // ---- loops ---------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn loops_of_face(
        &mut self,
        loop_start: i64,
        ptex: u32,
        face: usize,
        corner: usize,
        rotation: usize,
        v: [i64; 4],
        e: [i64; 4],
        u0: f32,
        v0: f32,
        du: f32,
        dv: f32,
    ) {
        let rot = |a: [i64; 4]| -> [i64; 4] {
            [
                a[(4 - rotation) % 4],
                a[(5 - rotation) % 4],
                a[(6 - rotation) % 4],
                a[(7 - rotation) % 4],
            ]
        };
        let (v, e) = (rot(v), rot(e));
        let coords = [(u0, v0), (u0 + du, v0), (u0 + du, v0 + dv), (u0, v0 + dv)];
        for k in 0..4 {
            let l = (loop_start + k as i64) as usize;
            self.out.corner_verts[l] = v[k] as u32;
            self.out.corner_edges[l] = e[k] as u32;
            self.out.corner_ptex[l] = (ptex, coords[k].0, coords[k].1);
            self.out.corner_data[l] = DataSource::Ptex {
                face: face as u32,
                corner: corner as u32,
                u: coords[k].0,
                v: coords[k].1,
            };
        }
        self.out.face_origin[(loop_start / 4) as usize] = face as u32;
    }

    fn loops_regular(&mut self, f: usize) {
        let res = self.resolution;
        let face = self.c.face(f);
        let n = face.len() as i64;
        let pr = ptex_face_resolution(face.len(), res);
        let pir = pr - 2;
        let per_edge_e = res - 1;
        let per_edge_v = res - 2;
        let inv = 1.0 / (pr - 1) as f32;
        let ptex = self.face_ptex_offset[f];
        let start_vert = self.vertices_inner_offset + self.subdiv_vert_offset[f];
        let start_edge = self.edge_inner_offset + self.subdiv_edge_offset[f];
        let start_face = self.subdiv_face_offset[f];
        let (du, dv) = (inv, inv);
        let mut l = 4 * start_face;
        for y in 1..pr - 2 {
            let v = y as f32 * inv;
            let iy = y - 1;
            for x in 1..pr - 2 {
                let ix = x - 1;
                let u = x as f32 * inv;
                let v0 = start_vert + (iy * pir + ix);
                let e0 = start_edge + (iy * (2 * pir - 1) + ix);
                let verts = [v0, v0 + 1, v0 + pir + 1, v0 + pir];
                let edges = [e0, e0 + pir, e0 + (2 * pir - 1), e0 + pir - 1];
                let ci = loops_corner_index(u, v, du, dv);
                self.loops_of_face(l, ptex, f, ci, 0, verts, edges, u, v, du, dv);
                l += 4;
            }
        }
        let mut prev_corner = n - 1;
        for corner in 0..n {
            let cvert = self.cv(face.start + corner as usize);
            let ce_i = self.ce(face.start + corner as usize);
            let prev_v = self.cv(face.start + prev_corner as usize);
            let prev_e = self.ce(face.start + prev_corner as usize);
            let ce = self.edge(ce_i);
            let pce = self.edge(prev_e);
            let start_edge_vert = self.vertices_edge_offset + ce_i * per_edge_v;
            let flip = ce[1] == cvert;
            let mut side_start = start_vert;
            let mut side_stride = 0;
            let mut v0 = self.vertices_corner_offset + cvert;
            let (mut v3, mut e3);
            if prev_v == pce[0] {
                v3 = self.vertices_edge_offset + prev_e * per_edge_v + per_edge_v - 1;
                e3 = self.edge_boundary_offset + prev_e * per_edge_e + per_edge_e - 1;
            } else {
                v3 = self.vertices_edge_offset + prev_e * per_edge_v;
                e3 = self.edge_boundary_offset + prev_e * per_edge_e;
            }
            let (mut e2_offset, mut e2_stride) = (0, 0);
            let (mut u, mut v, mut delta_u, mut delta_v) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            match corner {
                0 => {
                    side_stride = 1;
                    e2_offset = 0;
                    e2_stride = 1;
                    u = 0.0;
                    v = 0.0;
                    delta_u = du;
                    delta_v = 0.0;
                }
                1 => {
                    side_start += res - 3;
                    side_stride = res - 2;
                    e2_offset = 2 * per_edge_e - 4;
                    e2_stride = 2 * per_edge_e - 3;
                    u = 1.0 - du;
                    v = 0.0;
                    delta_u = 0.0;
                    delta_v = dv;
                }
                2 => {
                    side_start += per_edge_v * per_edge_v - 1;
                    side_stride = -1;
                    e2_offset = num_edges_per_ptex_face(res - 2) - 1;
                    e2_stride = -1;
                    u = 1.0 - du;
                    v = 1.0 - dv;
                    delta_u = -du;
                    delta_v = 0.0;
                }
                3 => {
                    side_start += per_edge_v * (per_edge_v - 1);
                    side_stride = -(res - 2);
                    e2_offset = num_edges_per_ptex_face(res - 2) - (2 * per_edge_e - 3);
                    e2_stride = -(2 * per_edge_e - 3);
                    u = 0.0;
                    v = 1.0 - dv;
                    delta_u = 0.0;
                    delta_v = -dv;
                }
                _ => {}
            }
            for i in 0..res - 2 {
                let v1 = if flip {
                    start_edge_vert + (res - i - 3)
                } else {
                    start_edge_vert + i
                };
                let v2 = side_start + side_stride * i;
                let e0 = if flip {
                    self.edge_boundary_offset + ce_i * per_edge_e + per_edge_e - i - 1
                } else {
                    self.edge_boundary_offset + ce_i * per_edge_e + i
                };
                let e1 = start_edge + num_edges_per_ptex_face(res - 2) + corner * per_edge_v + i;
                let e2 = if i == 0 {
                    start_edge
                        + num_edges_per_ptex_face(res - 2)
                        + ((corner - 1 + n) % n) * per_edge_v
                        + per_edge_v
                        - 1
                } else {
                    start_edge + e2_offset + e2_stride * (i - 1)
                };
                let lu = u + delta_u * i as f32;
                let lv = v + delta_v * i as f32;
                let ci = loops_corner_index(lu, lv, du, dv);
                self.loops_of_face(
                    l,
                    ptex,
                    f,
                    ci,
                    corner as usize,
                    [v0, v1, v2, v3],
                    [e0, e1, e2, e3],
                    lu,
                    lv,
                    du,
                    dv,
                );
                l += 4;
                v0 = v1;
                v3 = v2;
                e3 = e1;
            }
            prev_corner = corner;
        }
    }

    fn loops_special(&mut self, f: usize) {
        let res = self.resolution;
        let face = self.c.face(f);
        let n = face.len() as i64;
        let pr = ptex_face_resolution(face.len(), res);
        let pir = pr - 2;
        let inv = 1.0 / (pr - 1) as f32;
        let inner_verts_per_ptex = (pr - 1) * (pr - 2);
        let inner_edges_per_ptex = num_inner_edges_per_ptex_face(pir + 1);
        let per_edge_v = res - 2;
        let per_edge_e = res - 1;
        let ptex0 = self.face_ptex_offset[f];
        let center = self.vertices_inner_offset + self.subdiv_vert_offset[f];
        let start_vert = center + 1;
        let start_inner_vert = center + 1;
        let start_edge = self.edge_inner_offset + self.subdiv_edge_offset[f];
        let start_face = self.subdiv_face_offset[f];
        let (du, dv) = (inv, inv);
        let mut l = 4 * start_face;
        for corner in 0..n {
            let corner_vert = start_vert + corner * inner_verts_per_ptex;
            let corner_edge = start_edge + corner * inner_edges_per_ptex;
            for y in 1..pir {
                let v = y as f32 * inv;
                let iy = y - 1;
                for x in 1..pir + 1 {
                    let ix = x - 1;
                    let u = x as f32 * inv;
                    let v0 = corner_vert + (iy * (pir + 1) + ix);
                    let e0 = corner_edge + (iy * (2 * pir + 1) + ix);
                    let verts = [v0, v0 + 1, v0 + pir + 2, v0 + pir + 1];
                    let edges = [e0, e0 + pir + 1, e0 + (2 * pir + 1), e0 + pir];
                    self.loops_of_face(
                        l,
                        ptex0 + corner as u32,
                        f,
                        corner as usize,
                        0,
                        verts,
                        edges,
                        u,
                        v,
                        du,
                        dv,
                    );
                    l += 4;
                }
            }
        }
        for corner in 0..n {
            let next = (corner + 1) % n;
            let corner_edge = start_edge + corner * inner_edges_per_ptex;
            let next_corner_edge = start_edge + next * inner_edges_per_ptex;
            let mut cur = start_inner_vert + corner * inner_verts_per_ptex + pir;
            let mut nxt =
                start_inner_vert + next * inner_verts_per_ptex + inner_verts_per_ptex - pr + 1;
            let mut v0 = cur;
            let mut v1 = nxt;
            cur += pir + 1;
            nxt += 1;
            let mut e0 = start_edge + n * inner_edges_per_ptex + corner * (pr - 2);
            let mut e1 = next_corner_edge + inner_edges_per_ptex - pr + 2;
            let mut e3 = corner_edge + 2 * pr - 4;
            for row in 1..pir {
                let v2 = nxt;
                let v3 = cur;
                let e2 = e0 + 1;
                let u = row as f32 * du;
                let v = 1.0 - dv;
                self.loops_of_face(
                    l,
                    ptex0 + next as u32,
                    f,
                    next as usize,
                    3,
                    [v0, v1, v2, v3],
                    [e0, e1, e2, e3],
                    u,
                    v,
                    du,
                    dv,
                );
                l += 4;
                cur += pir + 1;
                nxt += 1;
                v0 = v3;
                v1 = v2;
                e0 = e2;
                e1 += 1;
                e3 += 2 * pr - 3;
            }
        }
        if pr >= 3 {
            let start_center_edge = start_edge + (inner_edges_per_ptex + pir) * n;
            let start_boundary_edge = start_edge + n * inner_edges_per_ptex + pir - 1;
            let mut prev = n - 1;
            for corner in 0..n {
                let corner_edge = start_edge + corner * inner_edges_per_ptex;
                let end = start_vert + corner * inner_verts_per_ptex + inner_verts_per_ptex - 1;
                let prev_end = start_vert + prev * inner_verts_per_ptex + inner_verts_per_ptex - 1;
                let verts = [center, prev_end, end - 1, end];
                let edges = [
                    start_center_edge + prev,
                    start_boundary_edge + prev * pir,
                    corner_edge + inner_edges_per_ptex - 1,
                    start_center_edge + corner,
                ];
                self.loops_of_face(
                    l,
                    ptex0 + corner as u32,
                    f,
                    corner as usize,
                    2,
                    verts,
                    edges,
                    1.0 - du,
                    1.0 - dv,
                    du,
                    dv,
                );
                l += 4;
                prev = corner;
            }
        }
        let mut prev = n - 1;
        for corner in 0..n {
            let cvert = self.cv(face.start + corner as usize);
            let ce_i = self.ce(face.start + corner as usize);
            let prev_v = self.cv(face.start + prev as usize);
            let prev_e = self.ce(face.start + prev as usize);
            let ce = self.edge(ce_i);
            let pce = self.edge(prev_e);
            let flip = ce[1] == cvert;
            let start_edge_vert = self.vertices_edge_offset + ce_i * per_edge_v;
            let corner_vert = start_vert + corner * inner_verts_per_ptex;
            let corner_edge = start_edge + corner * inner_edges_per_ptex;
            let mut v0 = self.vertices_corner_offset + cvert;
            let (mut v3, mut e3);
            if prev_v == pce[0] {
                v3 = self.vertices_edge_offset + prev_e * per_edge_v + per_edge_v - 1;
                e3 = self.edge_boundary_offset + prev_e * per_edge_e + per_edge_e - 1;
            } else {
                v3 = self.vertices_edge_offset + prev_e * per_edge_v;
                e3 = self.edge_boundary_offset + prev_e * per_edge_e;
            }
            for i in 0..=pir {
                let v1 = if flip {
                    start_edge_vert + (res - i - 3)
                } else {
                    start_edge_vert + i
                };
                let v2 = if pir >= 1 { corner_vert + i } else { center };
                let e0 = if flip {
                    self.edge_boundary_offset + ce_i * per_edge_e + per_edge_e - i - 1
                } else {
                    self.edge_boundary_offset + ce_i * per_edge_e + i
                };
                let mut e1 = start_edge + corner * (2 * pir + 1);
                if pr >= 3 {
                    e1 += n * (inner_edges_per_ptex + pir + 1) + i;
                }
                let e2 = if i == 0 && pr >= 3 {
                    start_edge
                        + n * (inner_edges_per_ptex + pir + 1)
                        + corner * (2 * pir + 1)
                        + pir
                        + 1
                } else if i == 0 {
                    start_edge + prev * (2 * pir + 1)
                } else {
                    corner_edge + i - 1
                };
                let u = du * i as f32;
                self.loops_of_face(
                    l,
                    ptex0 + corner as u32,
                    f,
                    corner as usize,
                    0,
                    [v0, v1, v2, v3],
                    [e0, e1, e2, e3],
                    u,
                    0.0,
                    du,
                    dv,
                );
                l += 4;
                v0 = v1;
                v3 = v2;
                e3 = e1;
            }
            let flip_prev = pce[1] == cvert;
            v0 = corner_vert;
            v3 = if prev_v == pce[0] {
                self.vertices_edge_offset + prev_e * per_edge_v + per_edge_v - 1
            } else {
                self.vertices_edge_offset + prev_e * per_edge_v
            };
            e3 = start_edge
                + n * (inner_edges_per_ptex + pir + 1)
                + corner * (2 * pir + 1)
                + pir
                + 1;
            for i in 0..pir {
                let (v1, e0, e1);
                if i == pir - 1 {
                    v1 = start_vert + prev * inner_verts_per_ptex + pir;
                    e1 = start_edge
                        + n * (inner_edges_per_ptex + pir + 1)
                        + prev * (2 * pir + 1)
                        + pir;
                    e0 = start_edge + n * inner_edges_per_ptex + prev * pir;
                } else {
                    v1 = v0 + pir + 1;
                    e0 = corner_edge + pir + i * (2 * pir + 1);
                    e1 = e3 + 1;
                }
                let v2 = if flip_prev { v3 - 1 } else { v3 + 1 };
                let e2 = if flip_prev {
                    self.edge_boundary_offset + prev_e * per_edge_e + per_edge_e - 2 - i
                } else {
                    self.edge_boundary_offset + prev_e * per_edge_e + 1 + i
                };
                let v = du * (i + 1) as f32;
                self.loops_of_face(
                    l,
                    ptex0 + corner as u32,
                    f,
                    corner as usize,
                    1,
                    [v0, v1, v2, v3],
                    [e0, e1, e2, e3],
                    0.0,
                    v,
                    du,
                    dv,
                );
                l += 4;
                v0 = v1;
                v3 = v2;
                e3 = e1;
            }
            prev = corner;
        }
    }

    // ---- loose geometry -------------------------------------------------------------------

    fn vertices_of_loose_edge(&mut self, ce_i: i64) {
        let res = self.resolution;
        let inv = 1.0 / (res - 1) as f32;
        let per_edge = res - 2;
        let ce = self.edge(ce_i);
        let v1 = self.vertices_corner_offset + ce[0];
        let v2 = self.vertices_corner_offset + ce[1];
        let start = self.vertices_edge_offset + ce_i * per_edge;
        for i in 0..res {
            let u = i as f32 * inv;
            let idx = if i == 0 {
                v1
            } else if i == res - 1 {
                v2
            } else {
                start + (i - 1)
            };
            let data = if u == 0.0 || u == 1.0 {
                DataSource::Keep
            } else {
                DataSource::Edge {
                    edge: ce_i as u32,
                    u,
                }
            };
            let i = idx as usize;
            self.out.vert_pos[i] = PosSource::LooseEdge {
                edge: ce_i as u32,
                u,
            };
            if data != DataSource::Keep {
                self.out.vert_data[i] = data;
            }
        }
    }
}

/// `subdiv_foreach_loops_corner_index`
fn loops_corner_index(u: f32, v: f32, du: f32, dv: f32) -> usize {
    if u + du <= 0.5 && v + dv <= 0.5 {
        0
    } else if u >= 0.5 && v + dv <= 0.5 {
        1
    } else if u >= 0.5 && v >= 0.5 {
        2
    } else {
        3
    }
}

/// Computes Blender's subdivided-mesh layout for `coarse` at `resolution`
/// (`foreach_subdiv_geometry` with the `subdiv_mesh.cc` callbacks).
pub fn compute(coarse: Coarse<'_>, resolution: u32) -> Layout {
    let res = resolution as i64;
    let nf = coarse.nfaces();
    let no_quad_res = (res >> 1) + 1;
    let per_edge_v = res - 2;
    let per_edge_e = res - 1;
    // `subdiv_foreach_ctx_init_offsets`
    let mut ctx = Ctx {
        c: coarse,
        resolution: res,
        vertices_corner_offset: 0,
        vertices_edge_offset: coarse.nverts as i64,
        vertices_inner_offset: coarse.nverts as i64 + coarse.edges.len() as i64 * per_edge_v,
        edge_boundary_offset: 0,
        edge_inner_offset: coarse.edges.len() as i64 * per_edge_e,
        subdiv_vert_offset: vec![0; nf],
        subdiv_edge_offset: vec![0; nf],
        subdiv_face_offset: vec![0; nf + 1],
        face_ptex_offset: Vec::with_capacity(nf + 1),
        verts_used: vec![false; coarse.nverts],
        edges_used: vec![false; coarse.edges.len()],
        out: Layout::default(),
    };
    let irregular_verts_per_patch = (no_quad_res - 2) * (no_quad_res - 1);
    let (mut vo, mut eo, mut fo) = (0i64, 0i64, 0i64);
    let mut ptex = 0u32;
    for f in 0..nf {
        let size = coarse.face(f).len();
        ctx.subdiv_vert_offset[f] = vo;
        ctx.subdiv_edge_offset[f] = eo;
        ctx.subdiv_face_offset[f] = fo;
        ctx.face_ptex_offset.push(ptex);
        ptex += num_ptex_faces_per_face(size) as u32;
        if size == 4 {
            vo += per_edge_v * per_edge_v;
            eo += num_edges_per_ptex_face(res - 2) + 4 * per_edge_v;
            fo += num_faces_per_ptex(res);
        } else {
            let n = size as i64;
            vo += 1 + n * irregular_verts_per_patch;
            eo += n
                * (num_inner_edges_per_ptex_face(no_quad_res - 1) + (no_quad_res - 2) + per_edge_v);
            if no_quad_res >= 3 {
                eo += n;
            }
            fo += n * num_faces_per_ptex(no_quad_res);
        }
    }
    ctx.subdiv_face_offset[nf] = fo;
    ctx.face_ptex_offset.push(ptex);
    // `subdiv_foreach_ctx_count`
    let nverts = ctx.vertices_inner_offset + vo;
    let nedges = ctx.edge_inner_offset + eo;
    let nfaces = fo;
    let nloops = 4 * nfaces;
    ctx.out = Layout {
        resolution,
        face_ptex_offset: ctx.face_ptex_offset.clone(),
        vert_pos: vec![PosSource::Coarse(0); nverts as usize],
        vert_data: vec![DataSource::Keep; nverts as usize],
        edges: vec![[0, 0]; nedges as usize],
        edge_origin: vec![None; nedges as usize],
        corner_verts: vec![0; nloops as usize],
        corner_edges: vec![0; nloops as usize],
        corner_ptex: vec![(0, 0.0, 0.0); nloops as usize],
        corner_data: vec![DataSource::Keep; nloops as usize],
        face_origin: vec![0; nfaces as usize],
    };
    // Single-threaded pass: coarse vertices and edge vertices through their first face.
    for f in 0..nf {
        if coarse.face(f).len() == 4 {
            ctx.corner_vertices_regular(f);
            ctx.edge_vertices_regular(f);
        } else {
            ctx.corner_vertices_special(f);
            ctx.edge_vertices_special(f);
        }
    }
    // Per face: inner vertices, edges, loops.
    for f in 0..nf {
        frap_math::budget::check_n(coarse.face(f).len() as u32 * resolution);
        if coarse.face(f).len() == 4 {
            ctx.inner_vertices_regular(f);
            ctx.edges_regular(f);
            ctx.loops_regular(f);
        } else {
            ctx.inner_vertices_special(f);
            ctx.edges_special(f);
            ctx.loops_special(f);
        }
    }
    // Loose vertices.
    for v in 0..coarse.nverts {
        if ctx.verts_used[v] {
            continue;
        }
        let i = ctx.vertices_corner_offset as usize + v;
        ctx.out.vert_pos[i] = PosSource::Coarse(v as u32);
        ctx.out.vert_data[i] = DataSource::Copy(v as u32);
    }
    // Vertices of loose edges.
    for e in 0..coarse.edges.len() {
        if ctx.edges_used[e] {
            continue;
        }
        ctx.vertices_of_loose_edge(e as i64);
    }
    // Boundary (coarse) edges.
    for e in 0..coarse.edges.len() {
        ctx.boundary_edges(e as i64);
    }
    ctx.out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_level1_counts_and_first_face() {
        // One quad 0-1-2-3, edges in face order.
        let edges = [[0, 1], [1, 2], [2, 3], [3, 0]];
        let l = compute(
            Coarse {
                nverts: 4,
                edges: &edges,
                face_offsets: &[0, 4],
                corner_verts: &[0, 1, 2, 3],
                corner_edges: &[0, 1, 2, 3],
            },
            3,
        );
        assert_eq!(l.vert_pos.len(), 9);
        assert_eq!(l.edges.len(), 12);
        assert_eq!(l.face_origin.len(), 4);
        // Blender's plane at level 1: [0, 4, 8, 7] is the first face.
        assert_eq!(&l.corner_verts[0..4], &[0, 4, 8, 7]);
    }

    #[test]
    fn ngon_counts() {
        let edges = [[0, 1], [1, 2], [2, 0]];
        let l = compute(
            Coarse {
                nverts: 3,
                edges: &edges,
                face_offsets: &[0, 3],
                corner_verts: &[0, 1, 2],
                corner_edges: &[0, 1, 2],
            },
            5,
        );
        // 3 corners + 3*3 edge verts + centre + 3 * (2*1) inner = 19.
        assert_eq!(l.vert_pos.len(), 3 + 9 + 1 + 6);
        assert_eq!(l.face_origin.len(), 12);
    }
}
