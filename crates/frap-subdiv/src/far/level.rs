// SPDX-License-Identifier: GPL-3.0-or-later
//! `Vtr::internal::Level`: one refinement level with all six topological relations in
//! OpenSubdiv's flat counts-and-offsets layout, the component tags and sharpness, the
//! face-varying channels, and the topology completion OpenSubdiv runs on the base level when
//! only face-vertices are given (Blender's converter never specifies full topology).
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/vtr/level.{h,cpp}` (Pixar, TOST / Apache-2.0
//! modified). Component order and incidence order are OpenSubdiv's, which the face-varying
//! stencils depend on bit for bit.

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::fvar_level::FVarLevel;
use super::tags::{ETag, RULE_SMOOTH, VTag};
use super::{INDEX_INVALID, Index, LocalIndex, VALENCE_LIMIT};

/// `Level::VSpan`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct VSpan {
    pub num_faces: LocalIndex,
    pub start_face: LocalIndex,
    pub corner_in_span: LocalIndex,
    pub periodic: bool,
    pub sharp: bool,
}

impl VSpan {
    /// `isAssigned`
    pub(crate) fn is_assigned(&self) -> bool {
        self.num_faces > 0
    }
}

/// One level of the refinement hierarchy.
#[derive(Clone, Debug, Default)]
pub(crate) struct Level {
    pub(crate) face_count: i32,
    pub(crate) edge_count: i32,
    pub(crate) vert_count: i32,
    pub(crate) depth: i32,
    pub(crate) max_edge_faces: i32,
    pub(crate) max_valence: i32,

    pub(crate) face_vert_co: Vec<Index>,
    pub(crate) face_vert_indices: Vec<Index>,
    pub(crate) face_edge_indices: Vec<Index>,
    pub(crate) face_holes: Vec<bool>,

    pub(crate) edge_vert_indices: Vec<Index>,
    pub(crate) edge_face_co: Vec<Index>,
    pub(crate) edge_face_indices: Vec<Index>,
    pub(crate) edge_face_local: Vec<LocalIndex>,
    pub(crate) edge_sharpness: Vec<f32>,
    pub(crate) edge_tags: Vec<ETag>,

    pub(crate) vert_face_co: Vec<Index>,
    pub(crate) vert_face_indices: Vec<Index>,
    pub(crate) vert_face_local: Vec<LocalIndex>,
    pub(crate) vert_edge_co: Vec<Index>,
    pub(crate) vert_edge_indices: Vec<Index>,
    pub(crate) vert_edge_local: Vec<LocalIndex>,
    pub(crate) vert_sharpness: Vec<f32>,
    pub(crate) vert_tags: Vec<VTag>,

    pub(crate) fvar: Vec<FVarLevel>,
}

#[inline]
fn range(co: &[Index], i: Index) -> std::ops::Range<usize> {
    let n = co[2 * i as usize] as usize;
    let o = co[2 * i as usize + 1] as usize;
    o..o + n
}

/// `fastMod4`
#[inline]
pub(crate) fn fast_mod4(x: i32) -> i32 {
    x & 3
}

impl Level {
    pub(crate) fn num_faces(&self) -> i32 {
        self.face_count
    }
    pub(crate) fn num_edges(&self) -> i32 {
        self.edge_count
    }

    pub(crate) fn num_face_vertices(&self, f: Index) -> i32 {
        self.face_vert_co[2 * f as usize]
    }
    pub(crate) fn offset_of_face_vertices(&self, f: Index) -> i32 {
        self.face_vert_co[2 * f as usize + 1]
    }

    pub(crate) fn face_vertices(&self, f: Index) -> &[Index] {
        &self.face_vert_indices[range(&self.face_vert_co, f)]
    }
    pub(crate) fn face_edges(&self, f: Index) -> &[Index] {
        &self.face_edge_indices[range(&self.face_vert_co, f)]
    }
    pub(crate) fn edge_vertices(&self, e: Index) -> [Index; 2] {
        [
            self.edge_vert_indices[2 * e as usize],
            self.edge_vert_indices[2 * e as usize + 1],
        ]
    }
    pub(crate) fn edge_faces(&self, e: Index) -> &[Index] {
        &self.edge_face_indices[range(&self.edge_face_co, e)]
    }
    pub(crate) fn edge_face_local_indices(&self, e: Index) -> &[LocalIndex] {
        &self.edge_face_local[range(&self.edge_face_co, e)]
    }
    pub(crate) fn vertex_faces(&self, v: Index) -> &[Index] {
        &self.vert_face_indices[range(&self.vert_face_co, v)]
    }
    pub(crate) fn vertex_face_local_indices(&self, v: Index) -> &[LocalIndex] {
        &self.vert_face_local[range(&self.vert_face_co, v)]
    }
    pub(crate) fn vertex_edges(&self, v: Index) -> &[Index] {
        &self.vert_edge_indices[range(&self.vert_edge_co, v)]
    }
    pub(crate) fn vertex_edge_local_indices(&self, v: Index) -> &[LocalIndex] {
        &self.vert_edge_local[range(&self.vert_edge_co, v)]
    }
    pub(crate) fn num_vertex_edges_total(&self) -> usize {
        self.vert_edge_indices.len()
    }

    pub(crate) fn vertex_tag(&self, v: Index) -> VTag {
        self.vert_tags[v as usize]
    }
    pub(crate) fn edge_tag(&self, e: Index) -> ETag {
        self.edge_tags[e as usize]
    }
    pub(crate) fn edge_sharpness(&self, e: Index) -> f32 {
        self.edge_sharpness[e as usize]
    }
    pub(crate) fn vertex_sharpness(&self, v: Index) -> f32 {
        self.vert_sharpness[v as usize]
    }
    pub(crate) fn vertex_rule(&self, v: Index) -> u8 {
        self.vert_tags[v as usize].rule()
    }
    pub(crate) fn is_face_hole(&self, f: Index) -> bool {
        self.face_holes[f as usize]
    }

    pub(crate) fn fvar_level(&self, c: usize) -> &FVarLevel {
        &self.fvar[c]
    }
    pub(crate) fn num_fvar_channels(&self) -> usize {
        self.fvar.len()
    }
    pub(crate) fn num_fvar_values(&self, c: usize) -> i32 {
        self.fvar[c].value_count
    }
    pub(crate) fn face_fvar_values(&self, f: Index, c: usize) -> &[Index] {
        &self.fvar[c].face_vert_values[range(&self.face_vert_co, f)]
    }

    // --- sizing (`resize*`, `trim*`) ---------------------------------------------------

    /// `resizeFaces`
    pub(crate) fn resize_faces(&mut self, n: i32) {
        self.face_count = n;
        self.face_vert_co.resize(2 * n as usize, 0);
        self.face_holes.clear();
        self.face_holes.resize(n as usize, false);
    }

    /// `resizeEdges`
    pub(crate) fn resize_edges(&mut self, n: i32) {
        self.edge_count = n;
        self.edge_face_co.resize(2 * n as usize, 0);
        self.edge_sharpness.resize(n as usize, 0.0);
        self.edge_tags.clear();
        self.edge_tags.resize(n as usize, ETag::default());
    }

    /// `resizeVertices`
    pub(crate) fn resize_vertices(&mut self, n: i32) {
        self.vert_count = n;
        self.vert_face_co.resize(2 * n as usize, 0);
        self.vert_edge_co.resize(2 * n as usize, 0);
        self.vert_sharpness.resize(n as usize, 0.0);
        self.vert_tags.clear();
        self.vert_tags.resize(n as usize, VTag::default());
    }

    /// `resizeFaceVertices(face, count)`
    pub(crate) fn resize_face_vertices_of(&mut self, f: Index, count: i32) {
        let i = 2 * f as usize;
        self.face_vert_co[i] = count;
        self.face_vert_co[i + 1] = if f == 0 {
            0
        } else {
            self.face_vert_co[i - 2] + self.face_vert_co[i - 1]
        };
        self.max_valence = self.max_valence.max(count);
    }

    /// `resizeEdgeFaces(edge, count)`
    pub(crate) fn resize_edge_faces_of(&mut self, e: Index, count: i32) {
        let i = 2 * e as usize;
        self.edge_face_co[i] = count;
        self.edge_face_co[i + 1] = if e == 0 {
            0
        } else {
            self.edge_face_co[i - 2] + self.edge_face_co[i - 1]
        };
        self.max_edge_faces = self.max_edge_faces.max(count);
    }

    /// `trimEdgeFaces`
    pub(crate) fn trim_edge_faces(&mut self, e: Index, count: i32) {
        self.edge_face_co[2 * e as usize] = count;
    }

    /// `resizeVertexFaces(vertex, count)`
    pub(crate) fn resize_vertex_faces_of(&mut self, v: Index, count: i32) {
        let i = 2 * v as usize;
        self.vert_face_co[i] = count;
        self.vert_face_co[i + 1] = if v == 0 {
            0
        } else {
            self.vert_face_co[i - 2] + self.vert_face_co[i - 1]
        };
    }

    /// `trimVertexFaces`
    pub(crate) fn trim_vertex_faces(&mut self, v: Index, count: i32) {
        self.vert_face_co[2 * v as usize] = count;
    }

    /// `resizeVertexEdges(vertex, count)`
    pub(crate) fn resize_vertex_edges_of(&mut self, v: Index, count: i32) {
        let i = 2 * v as usize;
        self.vert_edge_co[i] = count;
        self.vert_edge_co[i + 1] = if v == 0 {
            0
        } else {
            self.vert_edge_co[i - 2] + self.vert_edge_co[i - 1]
        };
        self.max_valence = self.max_valence.max(count);
    }

    /// `trimVertexEdges`
    pub(crate) fn trim_vertex_edges(&mut self, v: Index, count: i32) {
        self.vert_edge_co[2 * v as usize] = count;
    }

    pub(crate) fn num_edge_faces(&self, e: Index) -> i32 {
        self.edge_face_co[2 * e as usize]
    }
    pub(crate) fn offset_of_edge_faces(&self, e: Index) -> i32 {
        self.edge_face_co[2 * e as usize + 1]
    }
    pub(crate) fn num_vertex_faces(&self, v: Index) -> i32 {
        self.vert_face_co[2 * v as usize]
    }
    pub(crate) fn offset_of_vertex_faces(&self, v: Index) -> i32 {
        self.vert_face_co[2 * v as usize + 1]
    }
    pub(crate) fn num_vertex_edges(&self, v: Index) -> i32 {
        self.vert_edge_co[2 * v as usize]
    }
    pub(crate) fn offset_of_vertex_edges(&self, v: Index) -> i32 {
        self.vert_edge_co[2 * v as usize + 1]
    }

    // --- searches ------------------------------------------------------------------------

    /// `findEdge(v0, v1, v0Edges)`
    fn find_edge_in(&self, v0: Index, v1: Index, v0_edges: &[Index]) -> Index {
        if v0 != v1 {
            for &e in v0_edges {
                let ev = self.edge_vertices(e);
                if ev[0] == v1 || ev[1] == v1 {
                    return e;
                }
            }
        } else {
            for &e in v0_edges {
                let ev = self.edge_vertices(e);
                if ev[0] == ev[1] {
                    return e;
                }
            }
        }
        INDEX_INVALID
    }

    /// `findEdge(v0, v1)`
    pub(crate) fn find_edge(&self, v0: Index, v1: Index) -> Index {
        self.find_edge_in(v0, v1, self.vertex_edges(v0))
    }

    // --- base level topology completion -------------------------------------------------

    /// `completeTopologyFromFaceVertices`: the edges and all incident relations from the
    /// face-vertices. Returns `false` past `VALENCE_LIMIT`.
    pub(crate) fn complete_topology_from_face_vertices(&mut self) -> bool {
        let v_count = self.vert_count;
        let f_count = self.face_count;
        self.resize_vertices(v_count);
        self.resize_faces_keep_counts(f_count);
        self.resize_edges(0);
        self.face_edge_indices = vec![0; self.face_vert_indices.len()];
        self.edge_vert_indices.clear();

        let mut dyn_edge_faces: Vec<Vec<Index>> = Vec::new();
        let mut dyn_vert_faces: Vec<Vec<Index>> = vec![Vec::new(); v_count as usize];
        let mut dyn_vert_edges: Vec<Vec<Index>> = vec![Vec::new(); v_count as usize];
        let mut non_manifold_edges: Vec<Index> = Vec::new();

        for f in 0..f_count {
            let r = range(&self.face_vert_co, f);
            let n = r.len();
            frap_math::budget::check_n(n as u32);
            for i in 0..n {
                let v0 = self.face_vert_indices[r.start + i];
                let v1 = self.face_vert_indices[r.start + (i + 1) % n];
                let mut e = if v0 != v1 {
                    let mut found = INDEX_INVALID;
                    for &ce in &dyn_vert_edges[v0 as usize] {
                        let a = self.edge_vert_indices[2 * ce as usize];
                        let b = self.edge_vert_indices[2 * ce as usize + 1];
                        if a == v1 || b == v1 {
                            found = ce;
                            break;
                        }
                    }
                    found
                } else {
                    non_manifold_edges.push(self.edge_count);
                    INDEX_INVALID
                };
                if e != INDEX_INVALID {
                    let ef = &dyn_edge_faces[e as usize];
                    if *ef.last().expect("edge has a face") == f {
                        non_manifold_edges.push(e);
                        non_manifold_edges.push(self.edge_count);
                        e = INDEX_INVALID;
                    } else if ef.len() > 1 || v0 == self.edge_vert_indices[2 * e as usize] {
                        non_manifold_edges.push(e);
                    }
                }
                if e == INDEX_INVALID {
                    e = self.edge_count;
                    self.edge_count += 1;
                    self.edge_vert_indices.push(v0);
                    self.edge_vert_indices.push(v1);
                    dyn_edge_faces.push(Vec::new());
                    dyn_vert_edges[v0 as usize].push(e);
                    dyn_vert_edges[v1 as usize].push(e);
                }
                dyn_edge_faces[e as usize].push(f);
                dyn_vert_faces[v0 as usize].push(f);
                self.face_edge_indices[r.start + i] = e;
            }
        }

        let max_edge_faces = dyn_edge_faces.iter().map(Vec::len).max().unwrap_or(0) as i32;
        let max_vert_faces = dyn_vert_faces.iter().map(Vec::len).max().unwrap_or(0) as i32;
        let max_vert_edges = dyn_vert_edges.iter().map(Vec::len).max().unwrap_or(0) as i32;
        self.max_edge_faces = max_edge_faces;
        self.max_valence = self.max_valence.max(max_vert_faces).max(max_vert_edges);
        if self.max_valence > VALENCE_LIMIT {
            return false;
        }

        let e_count = self.edge_count;
        self.resize_edges(e_count);
        flatten(
            &dyn_edge_faces,
            &mut self.edge_face_co,
            &mut self.edge_face_indices,
        );
        flatten(
            &dyn_vert_faces,
            &mut self.vert_face_co,
            &mut self.vert_face_indices,
        );
        flatten(
            &dyn_vert_edges,
            &mut self.vert_edge_co,
            &mut self.vert_edge_indices,
        );

        for &e in &non_manifold_edges {
            self.edge_tags[e as usize].set_non_manifold(true);
            let ev = self.edge_vertices(e);
            self.vert_tags[ev[0] as usize].set_non_manifold(true);
            self.vert_tags[ev[1] as usize].set_non_manifold(true);
        }

        self.orient_incident_components();
        self.populate_local_indices();
        true
    }

    /// `resizeFaces` without clearing the face-vertex counts already assigned.
    fn resize_faces_keep_counts(&mut self, n: i32) {
        self.face_count = n;
        self.face_vert_co.resize(2 * n as usize, 0);
        self.face_holes.clear();
        self.face_holes.resize(n as usize, false);
    }

    /// `populateLocalIndices`
    pub(crate) fn populate_local_indices(&mut self) {
        self.vert_face_local = vec![0; self.vert_face_indices.len()];
        self.vert_edge_local = vec![0; self.vert_edge_indices.len()];
        self.edge_face_local = vec![0; self.edge_face_indices.len()];

        for v in 0..self.vert_count {
            let r = range(&self.vert_face_co, v);
            let mut last = INDEX_INVALID;
            for i in r.clone() {
                let f = self.vert_face_indices[i];
                let fv = self.face_vertices(f);
                let start = if f == last {
                    self.vert_face_local[i - 1] as usize + 1
                } else {
                    0
                };
                let k = fv[start..]
                    .iter()
                    .position(|&x| x == v)
                    .map_or(fv.len(), |p| p + start);
                self.vert_face_local[i] = k as LocalIndex;
                last = f;
            }
        }
        for v in 0..self.vert_count {
            let r = range(&self.vert_edge_co, v);
            for i in r.clone() {
                let e = self.vert_edge_indices[i];
                let ev = self.edge_vertices(e);
                self.vert_edge_local[i] = if ev[0] != ev[1] {
                    (v == ev[1]) as LocalIndex
                } else {
                    (i > r.start && self.vert_edge_indices[i] == self.vert_edge_indices[i - 1])
                        as LocalIndex
                };
            }
            self.max_valence = self.max_valence.max(r.len() as i32);
        }
        for e in 0..self.edge_count {
            let r = range(&self.edge_face_co, e);
            let mut last = INDEX_INVALID;
            for i in r.clone() {
                let f = self.edge_face_indices[i];
                let fe = self.face_edges(f);
                let start = if f == last {
                    self.edge_face_local[i - 1] as usize + 1
                } else {
                    0
                };
                let k = fe[start..]
                    .iter()
                    .position(|&x| x == e)
                    .map_or(fe.len(), |p| p + start);
                self.edge_face_local[i] = k as LocalIndex;
                last = f;
            }
        }
    }

    /// `orientIncidentComponents`
    fn orient_incident_components(&mut self) {
        for v in 0..self.vert_count {
            if !self.vert_tags[v as usize].non_manifold() && !self.order_vertex_faces_and_edges(v) {
                self.vert_tags[v as usize].set_non_manifold(true);
            }
        }
    }

    /// `orderVertexFacesAndEdges(vIndex)`
    fn order_vertex_faces_and_edges(&mut self, v: Index) -> bool {
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        if !self.order_vertex_faces_and_edges_into(v, &mut faces, &mut edges) {
            return false;
        }
        let rf = range(&self.vert_face_co, v);
        self.vert_face_indices[rf].copy_from_slice(&faces);
        let re = range(&self.vert_edge_co, v);
        self.vert_edge_indices[re].copy_from_slice(&edges);
        true
    }

    /// `orderVertexFacesAndEdges(vIndex, vFacesOrdered, vEdgesOrdered)`
    fn order_vertex_faces_and_edges_into(
        &self,
        v: Index,
        faces_out: &mut Vec<Index>,
        edges_out: &mut Vec<Index>,
    ) -> bool {
        let v_edges = self.vertex_edges(v);
        let v_faces = self.vertex_faces(v);
        let f_count = v_faces.len();
        let e_count = v_edges.len();
        if f_count == 0 || e_count < 2 || (e_count as i32 - f_count as i32) > 1 {
            return false;
        }
        let find = |a: &[Index], x: Index| a.iter().position(|&y| y == x).unwrap_or(a.len());
        let mut f_start;
        let mut e_start = INDEX_INVALID;
        let mut fv_start = 0usize;
        if e_count == f_count {
            f_start = v_faces[0];
            fv_start = find(self.face_vertices(f_start), v);
            e_start = self.face_edges(f_start)[fv_start];
        } else {
            f_start = INDEX_INVALID;
            for &e in v_edges {
                let ef = self.edge_faces(e);
                if ef.len() == 1 {
                    e_start = e;
                    f_start = ef[0];
                    fv_start = find(self.face_vertices(f_start), v);
                    if e_start == self.face_edges(f_start)[fv_start] {
                        break;
                    }
                }
            }
        }
        if f_start == INDEX_INVALID {
            return false;
        }
        faces_out.clear();
        edges_out.clear();
        faces_out.push(f_start);
        edges_out.push(e_start);
        let e_first = e_start;
        while edges_out.len() < e_count {
            let fv = self.face_vertices(f_start);
            let fe = self.face_edges(f_start);
            let fe_next = if fv_start > 0 {
                fv_start - 1
            } else {
                fv.len() - 1
            };
            let e_next = fe[fe_next];
            if e_next == e_start || e_next == e_first {
                return false;
            }
            edges_out.push(e_next);
            if faces_out.len() < f_count {
                let ef = self.edge_faces(e_next);
                if ef.is_empty() || (ef.len() == 1 && ef[0] == f_start) {
                    return false;
                }
                f_start = ef[(ef[0] == f_start) as usize];
                fv_start = find(self.face_edges(f_start), e_next);
                faces_out.push(f_start);
            }
            e_start = e_next;
        }
        faces_out.len() == f_count
    }

    /// `testVertexNonManifoldCrease`
    pub(crate) fn test_vertex_non_manifold_crease(&self, v: Index) -> bool {
        let mut nm = [-1 as Index, -1];
        for &e in self.vertex_edges(v) {
            if self.edge_tags[e as usize].non_manifold() {
                if nm[1] >= 0 {
                    return false;
                }
                let k = (nm[0] >= 0) as usize;
                nm[k] = e;
            } else if self.edge_faces(e).len() != 2 {
                return false;
            }
        }
        if nm[0] < 0 || nm[1] < 0 {
            return false;
        }
        if self.edge_faces(nm[0]).len() != self.edge_faces(nm[1]).len() {
            return false;
        }
        let mut traversed = 0usize;
        for i in 0..2 {
            let e_index = nm[i];
            let e_start = nm[(i != 0) as usize];
            let e_end = nm[(i == 0) as usize];
            let e_faces = self.edge_faces(e_index);
            let e_in_face = self.edge_face_local_indices(e_index);
            for j in 0..e_faces.len() {
                let f_start = e_faces[j];
                if self.face_vertices(f_start)[e_in_face[j] as usize] != v {
                    continue;
                }
                let mut e_next = e_start;
                let mut f_next = f_start;
                while e_next != e_end {
                    if e_next != e_start {
                        let pair = self.edge_faces(e_next);
                        f_next = pair[(pair[0] == f_next) as usize];
                    }
                    traversed += 1;
                    let fe = self.face_edges(f_next);
                    let i_next = fe.iter().position(|&x| x == e_next).unwrap_or(0);
                    e_next = fe[if i_next > 0 { i_next - 1 } else { fe.len() - 1 }];
                    if e_next == e_start {
                        return false;
                    }
                }
            }
        }
        traversed == self.vertex_faces(v).len()
    }

    // --- tags of faces -------------------------------------------------------------------

    /// `doesFaceFVarTopologyMatch`
    pub(crate) fn does_face_fvar_topology_match(&self, f: Index, c: usize) -> bool {
        !self.fvar[c].face_composite_value_tag(self, f).mismatch()
    }

    /// `getFaceVTags(fIndex, vTags, fvarChannel)`
    pub(crate) fn face_vtags(&self, f: Index, fvc: Option<usize>, out: &mut [VTag]) {
        let fv = self.face_vertices(f);
        match fvc {
            None => {
                for (o, &v) in out.iter_mut().zip(fv) {
                    *o = self.vert_tags[v as usize];
                }
            }
            Some(c) => {
                let fvar = &self.fvar[c];
                let vals = self.face_fvar_values(f, c);
                for i in 0..fv.len() {
                    let vi = fvar.find_vertex_value_index(self, fv[i], vals[i]);
                    out[i] = fvar.vert_value_tags[vi as usize]
                        .combine_with_level_vtag(self.vert_tags[fv[i] as usize]);
                }
            }
        }
    }

    /// `getFaceETags(fIndex, eTags, fvarChannel)`
    pub(crate) fn face_etags(&self, f: Index, fvc: Option<usize>, out: &mut [ETag]) {
        let fe = self.face_edges(f);
        for (o, &e) in out.iter_mut().zip(fe) {
            *o = match fvc {
                None => self.edge_tags[e as usize],
                Some(c) => self.fvar[c].edge_tags[e as usize]
                    .combine_with_level_etag(self.edge_tags[e as usize]),
            };
        }
    }

    /// `getFaceCompositeVTag(fIndex, -1)`
    pub(crate) fn face_composite_vtag(&self, f: Index) -> VTag {
        VTag(
            self.face_vertices(f)
                .iter()
                .fold(0, |a, &v| a | self.vert_tags[v as usize].0),
        )
    }

    /// `getVertexCompositeFVarVTag`
    pub(crate) fn vertex_composite_fvar_vtag(&self, v: Index, c: usize) -> VTag {
        let fvar = &self.fvar[c];
        let tags = fvar.vertex_value_tags(v);
        let vtag = self.vert_tags[v as usize];
        if tags[0].mismatch() {
            VTag(
                tags.iter()
                    .fold(0, |a, t| a | t.combine_with_level_vtag(vtag).0),
            )
        } else {
            vtag
        }
    }

    // --- patch point gathering -----------------------------------------------------------

    fn face_points(&self, f: Index, fvc: Option<usize>) -> &[Index] {
        match fvc {
            None => self.face_vertices(f),
            Some(c) => self.face_fvar_values(f, c),
        }
    }

    /// `gatherQuadRegularRingAroundVertex`
    pub(crate) fn gather_quad_regular_ring_around_vertex(
        &self,
        v: Index,
        ring: &mut Vec<Index>,
        fvc: Option<usize>,
    ) -> usize {
        ring.clear();
        let v_edges = self.vertex_edges(v);
        let v_faces = self.vertex_faces(v);
        let v_in_faces = self.vertex_face_local_indices(v);
        let is_boundary = v_edges.len() > v_faces.len();
        for i in 0..v_faces.len() {
            let fp = self.face_points(v_faces[i], fvc);
            let k = v_in_faces[i] as i32;
            ring.push(fp[fast_mod4(k + 1) as usize]);
            ring.push(fp[fast_mod4(k + 2) as usize]);
            if is_boundary && i == v_faces.len() - 1 {
                ring.push(fp[fast_mod4(k + 3) as usize]);
            }
        }
        ring.len()
    }
}

/// `DynamicRelation::compressMemberIndices` for one relation built per component.
fn flatten(lists: &[Vec<Index>], co: &mut Vec<Index>, idx: &mut Vec<Index>) {
    co.clear();
    idx.clear();
    for l in lists {
        co.push(l.len() as Index);
        co.push(idx.len() as Index);
        idx.extend_from_slice(l);
    }
}

/// The base-level vertex rule for a child vertex created from a face (`Refinement`).
pub(crate) fn smooth_vtag() -> VTag {
    let mut t = VTag::default();
    t.set_rule(RULE_SMOOTH);
    t
}
