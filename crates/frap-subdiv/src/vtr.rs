// SPDX-License-Identifier: GPL-3.0-or-later
//! Refinement levels (`Vtr::internal::Level`) and uniform quad refinement
//! (`Vtr::internal::QuadRefinement`) for the Catmull-Clark scheme.
//!
//! Ported from OpenSubdiv 3.x `opensubdiv/vtr/{level.cpp,refinement.cpp,quadRefinement.cpp}`
//! and `opensubdiv/far/topologyRefinerFactory.cpp` (`prepareComponentTagsAndSharpness`). The
//! topology of every level is completed from its face-vertices the way OpenSubdiv completes
//! the base level when only faces are given (`completeTopologyFromFaceVertices`, Blender's
//! converter never specifies full topology), so incident faces and edges of manifold vertices
//! are ordered counter-clockwise exactly as `orderVertexFacesAndEdges` orders them; component
//! tags and sharpness of child levels are propagated from their parents as `Refinement` does.

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use smallvec::SmallVec;

use crate::VtxBoundary;
use crate::sdc::{self, Rule};

/// Incident component lists (most vertices have few neighbours).
pub(crate) type Ring = SmallVec<[u32; 6]>;

/// `Level::ETag`
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct ETag {
    pub non_manifold: bool,
    pub boundary: bool,
    pub inf_sharp: bool,
    pub semi_sharp: bool,
}

/// `Level::VTag` (the members the scheme and the patch rules read).
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct VTag {
    pub non_manifold: bool,
    pub boundary: bool,
    pub inf_sharp: bool,
    pub semi_sharp: bool,
    pub semi_sharp_edges: bool,
    pub inf_sharp_edges: bool,
    pub rule: Rule,
}

/// One refinement level: complete topology, tags and sharpness.
#[derive(Clone, Debug, Default)]
pub(crate) struct Level {
    pub nverts: usize,
    /// `faces + 1` offsets into `face_verts` / `face_edges`.
    pub face_offsets: Vec<u32>,
    pub face_verts: Vec<u32>,
    /// Edge `i` of a face spans its vertices `i` and `i + 1`.
    pub face_edges: Vec<u32>,
    pub edge_verts: Vec<[u32; 2]>,
    pub edge_faces: Vec<SmallVec<[u32; 2]>>,
    pub vert_faces: Vec<Ring>,
    /// Index of the vertex in each incident face.
    pub vert_face_local: Vec<SmallVec<[u16; 6]>>,
    pub vert_edges: Vec<Ring>,
    /// Index (0/1) of the vertex in each incident edge.
    pub vert_edge_local: Vec<SmallVec<[u8; 6]>>,
    pub edge_tags: Vec<ETag>,
    pub vert_tags: Vec<VTag>,
    pub edge_sharpness: Vec<f32>,
    pub vert_sharpness: Vec<f32>,
}

impl Level {
    pub fn nfaces(&self) -> usize {
        self.face_offsets.len().saturating_sub(1)
    }

    pub fn nedges(&self) -> usize {
        self.edge_verts.len()
    }

    pub fn face_range(&self, f: usize) -> std::ops::Range<usize> {
        self.face_offsets[f] as usize..self.face_offsets[f + 1] as usize
    }

    pub fn face_verts(&self, f: usize) -> &[u32] {
        &self.face_verts[self.face_range(f)]
    }

    pub fn face_edges(&self, f: usize) -> &[u32] {
        &self.face_edges[self.face_range(f)]
    }

    /// The vertex at the other end of `e` from `v`.
    pub fn other_vert(&self, e: u32, v: u32) -> u32 {
        let ev = self.edge_verts[e as usize];
        if ev[0] == v { ev[1] } else { ev[0] }
    }

    /// `Level::findEdge`: the first edge incident to `v0` that also ends at `v1`.
    pub fn find_edge(&self, v0: u32, v1: u32) -> Option<u32> {
        if v0 == v1 {
            return None;
        }
        self.vert_edges[v0 as usize].iter().copied().find(|&e| {
            let ev = self.edge_verts[e as usize];
            ev[0] == v1 || ev[1] == v1
        })
    }

    /// `Level::completeTopologyFromFaceVertices` + `orientIncidentComponents` +
    /// `populateLocalIndices`. Non-manifold edges (more than two faces, opposite winding,
    /// repeated in a face) and their vertices are tagged `non_manifold`; vertices whose faces
    /// cannot be ordered into one fan are tagged too.
    pub fn from_faces(nverts: usize, face_offsets: Vec<u32>, face_verts: Vec<u32>) -> Level {
        let nfaces = face_offsets.len().saturating_sub(1);
        let mut lvl = Level {
            nverts,
            face_edges: vec![0; face_verts.len()],
            face_offsets,
            face_verts,
            vert_faces: vec![Ring::new(); nverts],
            vert_edges: vec![Ring::new(); nverts],
            vert_tags: vec![VTag::default(); nverts],
            vert_sharpness: vec![0.0; nverts],
            ..Level::default()
        };
        let mut non_manifold_edges: Vec<u32> = Vec::new();
        for f in 0..nfaces {
            let r = lvl.face_range(f);
            let n = r.len();
            frap_math::budget::check_n(n as u32);
            for i in 0..n {
                let v0 = lvl.face_verts[r.start + i];
                let v1 = lvl.face_verts[r.start + (i + 1) % n];
                let mut e = if v0 != v1 {
                    lvl.find_edge(v0, v1)
                } else {
                    non_manifold_edges.push(lvl.edge_verts.len() as u32);
                    None
                };
                if let Some(ei) = e {
                    let ef = &lvl.edge_faces[ei as usize];
                    if *ef.last().expect("edge has a face") == f as u32 {
                        non_manifold_edges.push(ei);
                        non_manifold_edges.push(lvl.edge_verts.len() as u32);
                        e = None;
                    } else if ef.len() > 1 || v0 == lvl.edge_verts[ei as usize][0] {
                        non_manifold_edges.push(ei);
                    }
                }
                let ei = match e {
                    Some(ei) => ei,
                    None => {
                        let ei = lvl.edge_verts.len() as u32;
                        lvl.edge_verts.push([v0, v1]);
                        lvl.edge_faces.push(SmallVec::new());
                        lvl.vert_edges[v0 as usize].push(ei);
                        lvl.vert_edges[v1 as usize].push(ei);
                        ei
                    }
                };
                lvl.edge_faces[ei as usize].push(f as u32);
                lvl.vert_faces[v0 as usize].push(f as u32);
                lvl.face_edges[r.start + i] = ei;
            }
        }
        let ne = lvl.edge_verts.len();
        lvl.edge_tags = vec![ETag::default(); ne];
        lvl.edge_sharpness = vec![0.0; ne];
        for &e in &non_manifold_edges {
            lvl.edge_tags[e as usize].non_manifold = true;
            let ev = lvl.edge_verts[e as usize];
            lvl.vert_tags[ev[0] as usize].non_manifold = true;
            lvl.vert_tags[ev[1] as usize].non_manifold = true;
        }
        for v in 0..nverts {
            if !lvl.vert_tags[v].non_manifold && !lvl.order_vertex_faces_and_edges(v) {
                lvl.vert_tags[v].non_manifold = true;
            }
        }
        lvl.populate_local_indices();
        lvl
    }

    fn populate_local_indices(&mut self) {
        let mut vfl = Vec::with_capacity(self.nverts);
        for v in 0..self.nverts {
            let mut loc: SmallVec<[u16; 6]> = SmallVec::new();
            let mut last: Option<u32> = None;
            for (i, &f) in self.vert_faces[v].iter().enumerate() {
                let fv = self.face_verts(f as usize);
                let start = if last == Some(f) {
                    loc[i - 1] as usize + 1
                } else {
                    0
                };
                let k = fv[start..]
                    .iter()
                    .position(|&x| x == v as u32)
                    .map_or(fv.len(), |p| p + start);
                loc.push(k as u16);
                last = Some(f);
            }
            vfl.push(loc);
        }
        self.vert_face_local = vfl;
        let mut vel = Vec::with_capacity(self.nverts);
        for v in 0..self.nverts {
            let edges = &self.vert_edges[v];
            let mut loc: SmallVec<[u8; 6]> = SmallVec::new();
            for (i, &e) in edges.iter().enumerate() {
                let ev = self.edge_verts[e as usize];
                if ev[0] != ev[1] {
                    loc.push((v as u32 == ev[1]) as u8);
                } else {
                    loc.push((i > 0 && edges[i] == edges[i - 1]) as u8);
                }
            }
            vel.push(loc);
        }
        self.vert_edge_local = vel;
    }

    /// `Level::orderVertexFacesAndEdges`: counter-clockwise faces and edges around a manifold
    /// vertex, edge `i` leading face `i`; a boundary vertex starts at the boundary edge that
    /// leads its face. `false` when the vertex is not a single manifold fan.
    fn order_vertex_faces_and_edges(&mut self, v: usize) -> bool {
        let vfaces = self.vert_faces[v].clone();
        let vedges = self.vert_edges[v].clone();
        let fcount = vfaces.len();
        let ecount = vedges.len();
        if fcount == 0 || ecount < 2 || ecount.saturating_sub(fcount) > 1 || fcount > ecount {
            return false;
        }
        let find = |a: &[u32], x: u32| a.iter().position(|&y| y == x).unwrap_or(a.len());
        let (mut f_start, mut e_start, mut fv_start);
        if ecount == fcount {
            f_start = vfaces[0];
            fv_start = find(self.face_verts(f_start as usize), v as u32);
            e_start = self.face_edges(f_start as usize)[fv_start];
        } else {
            f_start = u32::MAX;
            e_start = u32::MAX;
            fv_start = 0;
            for &e in &vedges {
                let ef = &self.edge_faces[e as usize];
                if ef.len() == 1 {
                    e_start = e;
                    f_start = ef[0];
                    fv_start = find(self.face_verts(f_start as usize), v as u32);
                    if e_start == self.face_edges(f_start as usize)[fv_start] {
                        break;
                    }
                }
            }
            if f_start == u32::MAX {
                return false;
            }
        }
        let mut faces_o: Ring = SmallVec::new();
        let mut edges_o: Ring = SmallVec::new();
        faces_o.push(f_start);
        edges_o.push(e_start);
        let e_first = e_start;
        while edges_o.len() < ecount {
            let fedges = self.face_edges(f_start as usize);
            let n = fedges.len();
            let fe_next = if fv_start > 0 { fv_start - 1 } else { n - 1 };
            let e_next = fedges[fe_next];
            if e_next == e_start || e_next == e_first {
                return false;
            }
            edges_o.push(e_next);
            if faces_o.len() < fcount {
                let ef = &self.edge_faces[e_next as usize];
                if ef.is_empty() || (ef.len() == 1 && ef[0] == f_start) {
                    return false;
                }
                f_start = ef[(ef[0] == f_start) as usize];
                fv_start = find(self.face_edges(f_start as usize), e_next);
                faces_o.push(f_start);
            }
            e_start = e_next;
        }
        if faces_o.len() != fcount {
            return false;
        }
        self.vert_faces[v] = faces_o;
        self.vert_edges[v] = edges_o;
        true
    }

    /// `Level::testVertexNonManifoldCrease`
    fn test_vertex_non_manifold_crease(&self, v: usize) -> bool {
        let mut nm = [u32::MAX; 2];
        for &e in &self.vert_edges[v] {
            if self.edge_tags[e as usize].non_manifold {
                if nm[1] != u32::MAX {
                    return false;
                }
                let slot = (nm[0] != u32::MAX) as usize;
                nm[slot] = e;
            } else if self.edge_faces[e as usize].len() != 2 {
                return false;
            }
        }
        if nm[0] == u32::MAX || nm[1] == u32::MAX {
            return false;
        }
        if self.edge_faces[nm[0] as usize].len() != self.edge_faces[nm[1] as usize].len() {
            return false;
        }
        let mut traversed = 0usize;
        for i in 0..2 {
            let e = nm[i];
            let e_start = nm[(i != 0) as usize];
            let e_end = nm[(i == 0) as usize];
            for &f_start in &self.edge_faces[e as usize] {
                let fv = self.face_verts(f_start as usize);
                let fe = self.face_edges(f_start as usize);
                let in_face = fe.iter().position(|&x| x == e).unwrap_or(0);
                if fv[in_face] != v as u32 {
                    continue;
                }
                let (mut e_next, mut f_next) = (e_start, f_start);
                while e_next != e_end {
                    if e_next != e_start {
                        let pair = &self.edge_faces[e_next as usize];
                        f_next = pair[(pair[0] == f_next) as usize];
                    }
                    traversed += 1;
                    let fe = self.face_edges(f_next as usize);
                    let i_next = fe.iter().position(|&x| x == e_next).unwrap_or(0);
                    e_next = fe[if i_next > 0 { i_next - 1 } else { fe.len() - 1 }];
                    if e_next == e_start {
                        return false;
                    }
                }
            }
        }
        traversed == self.vert_faces[v].len()
    }

    /// `TopologyRefinerFactoryBase::prepareComponentTagsAndSharpness` for the Catmark scheme:
    /// boundary and non-manifold edges become infinitely sharp, corners are sharpened with
    /// `EDGE_AND_CORNER`, and every vertex gets its rule.
    pub fn prepare_base_tags(&mut self, vtx_boundary: VtxBoundary) {
        let sharpen_corners = vtx_boundary == VtxBoundary::EdgeAndCorner;
        for e in 0..self.nedges() {
            let t = &mut self.edge_tags[e];
            let s = &mut self.edge_sharpness[e];
            t.boundary = self.edge_faces[e].len() < 2;
            if t.boundary || t.non_manifold {
                *s = sdc::SHARPNESS_INFINITE;
            }
            t.inf_sharp = sdc::is_infinite(*s);
            t.semi_sharp = sdc::is_sharp(*s) && !t.inf_sharp;
        }
        for v in 0..self.nverts {
            let (mut boundary, mut inf, mut semi, mut nonman) = (0usize, 0usize, 0usize, 0usize);
            for &e in &self.vert_edges[v] {
                let t = self.edge_tags[e as usize];
                boundary += t.boundary as usize;
                inf += t.inf_sharp as usize;
                semi += t.semi_sharp as usize;
                nonman += t.non_manifold as usize;
            }
            let nfaces = self.vert_faces[v].len();
            let nedges = self.vert_edges[v].len();
            let topological_corner = nfaces == 1 && nedges == 2;
            if topological_corner && sharpen_corners {
                self.vert_sharpness[v] = sdc::SHARPNESS_INFINITE;
            } else if self.vert_tags[v].non_manifold && !sdc::is_infinite(self.vert_sharpness[v]) {
                let non_man_crease = nonman == 2
                    && boundary == 0
                    && nfaces > nedges
                    && self.test_vertex_non_manifold_crease(v);
                if !non_man_crease {
                    self.vert_sharpness[v] = sdc::SHARPNESS_INFINITE;
                }
            }
            let s = self.vert_sharpness[v];
            let t = &mut self.vert_tags[v];
            t.inf_sharp = sdc::is_infinite(s);
            t.semi_sharp = sdc::is_semi_sharp(s);
            t.semi_sharp_edges = semi > 0;
            t.inf_sharp_edges = inf > 0;
            t.rule = Rule::determine(s, inf + semi);
            t.boundary = boundary > 0;
        }
    }

    /// Sharpness of the edges around `v`, in `vert_edges` order.
    pub fn vert_edge_sharpness(&self, v: usize) -> SmallVec<[f32; 6]> {
        self.vert_edges[v]
            .iter()
            .map(|&e| self.edge_sharpness[e as usize])
            .collect()
    }
}

/// Parent-to-child mapping of one uniform quad refinement.
#[derive(Clone, Debug, Default)]
pub(crate) struct Refinement {
    pub face_child_vert: Vec<u32>,
    pub edge_child_vert: Vec<u32>,
    pub vert_child_vert: Vec<u32>,
    /// Child edges of each parent edge: `[at edge vertex 0, at edge vertex 1]`.
    pub edge_child_edges: Vec<[u32; 2]>,
}

impl Refinement {
    /// Child face `k` of parent face `f` (one child face per parent face-corner, in order).
    #[inline]
    pub fn face_child_face(parent: &Level, f: usize, k: usize) -> usize {
        parent.face_offsets[f] as usize + k
    }
}

/// Refines `parent` once (Catmull-Clark topology split): returns the child level, its
/// topology complete and its tags and sharpness propagated as `Vtr::internal::Refinement`
/// does (`propagateComponentTags`, `subdivideSharpnessValues`,
/// `reclassifySemisharpVertices`).
pub(crate) fn refine(parent: &Level) -> (Level, Refinement) {
    let nf = parent.nfaces();
    let ne = parent.nedges();
    let nv = parent.nverts;
    let face_child_vert: Vec<u32> = (0..nf as u32).collect();
    let edge_child_vert: Vec<u32> = (0..ne as u32).map(|e| nf as u32 + e).collect();
    let vert_child_vert: Vec<u32> = (0..nv as u32).map(|v| (nf + ne) as u32 + v).collect();
    let mut offsets = Vec::with_capacity(parent.face_verts.len() + 1);
    let mut verts = Vec::with_capacity(parent.face_verts.len() * 4);
    offsets.push(0u32);
    for f in 0..nf {
        let fv = parent.face_verts(f);
        let fe = parent.face_edges(f);
        let n = fv.len();
        for k in 0..n {
            verts.push(vert_child_vert[fv[k] as usize]);
            verts.push(edge_child_vert[fe[k] as usize]);
            verts.push(face_child_vert[f]);
            verts.push(edge_child_vert[fe[(k + n - 1) % n] as usize]);
            offsets.push(verts.len() as u32);
        }
    }
    let mut child = Level::from_faces(nf + ne + nv, offsets, verts);
    let mut edge_child_edges = vec![[u32::MAX; 2]; ne];
    for e in 0..ne {
        let ev = parent.edge_verts[e];
        let c = edge_child_vert[e];
        for k in 0..2 {
            if let Some(ce) = child.find_edge(c, vert_child_vert[ev[k] as usize]) {
                edge_child_edges[e][k] = ce;
            }
        }
    }
    // Edge tags and sharpness: edges from faces are smooth; edges from edges inherit.
    for t in child.edge_tags.iter_mut() {
        *t = ETag::default();
    }
    for s in child.edge_sharpness.iter_mut() {
        *s = 0.0;
    }
    for (e, pair) in edge_child_edges.iter().enumerate() {
        let pt = parent.edge_tags[e];
        let ps = parent.edge_sharpness[e];
        for &ce in pair {
            if ce == u32::MAX {
                continue;
            }
            let mut t = pt;
            let s = if t.inf_sharp {
                sdc::SHARPNESS_INFINITE
            } else if t.semi_sharp {
                let s = sdc::decrement_sharpness(ps);
                if !sdc::is_sharp(s) {
                    t.semi_sharp = false;
                }
                s
            } else {
                0.0
            };
            child.edge_tags[ce as usize] = t;
            child.edge_sharpness[ce as usize] = s;
        }
    }
    // Vertex tags and sharpness.
    for v in 0..child.nverts {
        child.vert_tags[v] = VTag::default();
        child.vert_sharpness[v] = 0.0;
    }
    for &cv in &face_child_vert {
        child.vert_tags[cv as usize].rule = Rule::Smooth;
    }
    for e in 0..ne {
        let pt = parent.edge_tags[e];
        let t = &mut child.vert_tags[edge_child_vert[e] as usize];
        t.non_manifold = pt.non_manifold;
        t.boundary = pt.boundary;
        t.semi_sharp_edges = pt.semi_sharp;
        t.inf_sharp_edges = pt.inf_sharp;
        t.rule = if pt.semi_sharp || pt.inf_sharp {
            Rule::Crease
        } else {
            Rule::Smooth
        };
    }
    for v in 0..nv {
        let cv = vert_child_vert[v] as usize;
        let mut t = parent.vert_tags[v];
        let s = if t.inf_sharp {
            sdc::SHARPNESS_INFINITE
        } else if t.semi_sharp {
            let s = sdc::decrement_sharpness(parent.vert_sharpness[v]);
            if !sdc::is_sharp(s) {
                t.semi_sharp = false;
            }
            s
        } else {
            0.0
        };
        child.vert_tags[cv] = t;
        child.vert_sharpness[cv] = s;
    }
    // `reclassifySemisharpVertices`
    for e in 0..ne {
        let cv = edge_child_vert[e] as usize;
        if !child.vert_tags[cv].semi_sharp_edges {
            continue;
        }
        let count = edge_child_edges[e]
            .iter()
            .filter(|&&ce| ce != u32::MAX && child.edge_tags[ce as usize].semi_sharp)
            .count();
        let t = &mut child.vert_tags[cv];
        t.semi_sharp_edges = count > 0;
        t.rule = Rule::determine(0.0, count);
    }
    for v in 0..nv {
        let pt = parent.vert_tags[v];
        if !pt.semi_sharp && !pt.semi_sharp_edges {
            continue;
        }
        let cv = vert_child_vert[v] as usize;
        let decayed = pt.semi_sharp && !child.vert_tags[cv].semi_sharp;
        if pt.semi_sharp_edges || decayed {
            let (mut inf, mut semi) = (0usize, 0usize);
            for (i, &pe) in parent.vert_edges[v].iter().enumerate() {
                let k = parent.vert_edge_local[v][i] as usize;
                let ce = edge_child_edges[pe as usize][k];
                if ce == u32::MAX {
                    continue;
                }
                let t = child.edge_tags[ce as usize];
                inf += t.inf_sharp as usize;
                semi += t.semi_sharp as usize;
            }
            let t = &mut child.vert_tags[cv];
            t.semi_sharp_edges = semi > 0;
            if !t.semi_sharp && !t.inf_sharp {
                t.rule = Rule::determine(0.0, inf + semi);
            }
        }
    }
    (
        child,
        Refinement {
            face_child_vert,
            edge_child_vert,
            vert_child_vert,
            edge_child_edges,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Level {
        let faces: [[u32; 4]; 6] = [
            [0, 1, 3, 2],
            [2, 3, 5, 4],
            [4, 5, 7, 6],
            [6, 7, 1, 0],
            [1, 7, 5, 3],
            [6, 0, 2, 4],
        ];
        let mut off = vec![0u32];
        let mut fv = Vec::new();
        for f in faces {
            fv.extend_from_slice(&f);
            off.push(fv.len() as u32);
        }
        Level::from_faces(8, off, fv)
    }

    #[test]
    fn cube_topology_is_closed_and_ordered() {
        let mut c = cube();
        c.prepare_base_tags(VtxBoundary::EdgeOnly);
        assert_eq!(c.nedges(), 12);
        for v in 0..8 {
            assert_eq!(c.vert_faces[v].len(), 3);
            assert!(!c.vert_tags[v].non_manifold);
            assert_eq!(c.vert_tags[v].rule, Rule::Smooth);
            // Edge i leads face i: it is the face edge starting at the vertex.
            for (i, &f) in c.vert_faces[v].iter().enumerate() {
                let k = c.vert_face_local[v][i] as usize;
                assert_eq!(c.face_edges(f as usize)[k], c.vert_edges[v][i]);
            }
        }
        let (child, r) = refine(&c);
        assert_eq!(child.nfaces(), 24);
        assert_eq!(child.nedges(), 48);
        assert_eq!(child.nverts, 26);
        assert!(
            r.edge_child_edges
                .iter()
                .all(|p| p[0] != u32::MAX && p[1] != u32::MAX)
        );
    }

    #[test]
    fn plane_boundary_rules() {
        let mut p = Level::from_faces(4, vec![0, 4], vec![0, 1, 2, 3]);
        p.prepare_base_tags(VtxBoundary::EdgeOnly);
        assert!(p.vert_tags.iter().all(|t| t.rule == Rule::Crease));
        let mut p = Level::from_faces(4, vec![0, 4], vec![0, 1, 2, 3]);
        p.prepare_base_tags(VtxBoundary::EdgeAndCorner);
        assert!(p.vert_tags.iter().all(|t| t.rule == Rule::Corner));
    }
}
