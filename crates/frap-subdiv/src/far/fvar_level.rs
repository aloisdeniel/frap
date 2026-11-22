// SPDX-License-Identifier: GPL-3.0-or-later
//! `Vtr::internal::FVarLevel`: one face-varying channel on one level — the values of every
//! face corner, the "sibling" values around each vertex, and their topological tags under the
//! channel's linear-interpolation option.
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/vtr/fvarLevel.{h,cpp}` (Pixar, TOST / Apache-2.0
//! modified). At level 0 the face values are the client's value indices and every vertex lists
//! its siblings' values; on refined levels the face values are the vertex-value offsets.

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::level::Level;
use super::sdc::{FVarLinear, SdcOptions, VtxBoundary};
use super::tags::{FvETag, ValueTag};
use super::{Index, LocalIndex};

/// `FVarLevel::CreaseEndPair`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CreaseEndPair {
    pub start_face: LocalIndex,
    pub end_face: LocalIndex,
}

/// `FVarLevel::ValueSpan`
#[derive(Copy, Clone, Debug, Default)]
struct ValueSpan {
    size: LocalIndex,
    start: LocalIndex,
    discts_edge_count: LocalIndex,
    semi_sharp_edge_count: LocalIndex,
    inf_sharp_edge_count: LocalIndex,
}

/// One face-varying channel of a [`Level`].
#[derive(Clone, Debug, Default)]
pub(crate) struct FVarLevel {
    pub(crate) options: SdcOptions,
    pub(crate) is_linear: bool,
    pub(crate) has_linear_boundaries: bool,
    pub(crate) has_dependent_sharpness: bool,
    pub(crate) value_count: i32,
    pub(crate) face_vert_values: Vec<Index>,
    pub(crate) edge_tags: Vec<FvETag>,
    pub(crate) vert_sibling_counts: Vec<LocalIndex>,
    pub(crate) vert_sibling_offsets: Vec<i32>,
    pub(crate) vert_face_siblings: Vec<LocalIndex>,
    pub(crate) vert_value_indices: Vec<Index>,
    pub(crate) vert_value_tags: Vec<ValueTag>,
    pub(crate) vert_value_crease_ends: Vec<CreaseEndPair>,
}

impl FVarLevel {
    /// `hasSmoothBoundaries` (`hasCreaseEnds`)
    pub(crate) fn has_smooth_boundaries(&self) -> bool {
        !self.has_linear_boundaries
    }

    /// `resizeComponents` for `level`.
    pub(crate) fn resize_components(&mut self, level: &Level) {
        self.face_vert_values = vec![0; level.face_vert_indices.len()];
        self.edge_tags = vec![FvETag::default(); level.edge_count as usize];
        self.vert_sibling_counts = vec![0; level.vert_count as usize];
        self.vert_sibling_offsets = vec![0; level.vert_count as usize];
        self.vert_face_siblings = vec![0; level.vert_face_indices.len()];
    }

    /// `resizeVertexValues`
    fn resize_vertex_values(&mut self, n: usize) {
        self.vert_value_indices.resize(n, 0);
        self.vert_value_tags = vec![ValueTag::default(); n];
        if self.has_smooth_boundaries() {
            self.vert_value_crease_ends
                .resize(n, CreaseEndPair::default());
        }
    }

    pub(crate) fn num_vertex_values(&self, v: Index) -> i32 {
        self.vert_sibling_counts[v as usize] as i32
    }
    pub(crate) fn vertex_value_offset(&self, v: Index, sibling: i32) -> Index {
        self.vert_sibling_offsets[v as usize] + sibling
    }
    pub(crate) fn vertex_value(&self, v: Index, sibling: i32) -> Index {
        self.vert_value_indices[self.vertex_value_offset(v, sibling) as usize]
    }
    pub(crate) fn vertex_values(&self, v: Index) -> &[Index] {
        let o = self.vert_sibling_offsets[v as usize] as usize;
        &self.vert_value_indices[o..o + self.vert_sibling_counts[v as usize] as usize]
    }
    pub(crate) fn vertex_value_tags(&self, v: Index) -> &[ValueTag] {
        let o = self.vert_sibling_offsets[v as usize] as usize;
        &self.vert_value_tags[o..o + self.vert_sibling_counts[v as usize] as usize]
    }
    pub(crate) fn vertex_value_crease_ends(&self, v: Index) -> &[CreaseEndPair] {
        let o = self.vert_sibling_offsets[v as usize] as usize;
        &self.vert_value_crease_ends[o..o + self.vert_sibling_counts[v as usize] as usize]
    }
    pub(crate) fn vertex_face_siblings<'a>(&'a self, level: &Level, v: Index) -> &'a [LocalIndex] {
        let o = level.offset_of_vertex_faces(v) as usize;
        &self.vert_face_siblings[o..o + level.num_vertex_faces(v) as usize]
    }
    pub(crate) fn value_topology_matches(&self, value_index: Index) -> bool {
        !self.vert_value_tags[value_index as usize].mismatch()
    }
    pub(crate) fn edge_topology_matches(&self, e: Index) -> bool {
        !self.edge_tags[e as usize].mismatch()
    }
    pub(crate) fn face_values<'a>(&'a self, level: &Level, f: Index) -> &'a [Index] {
        let o = level.offset_of_face_vertices(f) as usize;
        &self.face_vert_values[o..o + level.num_face_vertices(f) as usize]
    }

    /// `findVertexValueIndex`
    pub(crate) fn find_vertex_value_index(&self, level: &Level, v: Index, value: Index) -> Index {
        if level.depth > 0 {
            return value;
        }
        let mut vv = self.vertex_value_offset(v, 0);
        while self.vert_value_indices[vv as usize] != value {
            vv += 1;
        }
        vv
    }

    /// `getFaceValueTags`
    pub(crate) fn face_value_tags(&self, level: &Level, f: Index, out: &mut [ValueTag]) {
        let vals = self.face_values(level, f);
        let verts = level.face_vertices(f);
        for i in 0..vals.len() {
            let vi = self.find_vertex_value_index(level, verts[i], vals[i]);
            out[i] = self.vert_value_tags[vi as usize];
        }
    }

    /// `getFaceCompositeValueTag`
    pub(crate) fn face_composite_value_tag(&self, level: &Level, f: Index) -> ValueTag {
        let vals = self.face_values(level, f);
        let verts = level.face_vertices(f);
        let mut bits = 0u8;
        for i in 0..vals.len() {
            let vi = self.find_vertex_value_index(level, verts[i], vals[i]);
            bits |= self.vert_value_tags[vi as usize].0;
        }
        ValueTag(bits)
    }

    /// `completeTopologyFromFaceValues`
    pub(crate) fn complete_topology_from_face_values(
        &mut self,
        level: &Level,
        regular_boundary_valence: i32,
    ) {
        let geom = self.options.vtx_boundary;
        let fvar = self.options.fvar_linear;
        self.is_linear = fvar == FVarLinear::All;
        self.has_linear_boundaries = fvar == FVarLinear::All || fvar == FVarLinear::Boundaries;
        self.has_dependent_sharpness =
            fvar == FVarLinear::CornersPlus1 || fvar == FVarLinear::CornersPlus2;
        let geom_corners_are_smooth = geom != VtxBoundary::EdgeAndCorner;
        let fvar_corners_are_sharp = fvar != FVarLinear::None;
        let make_smooth_corners_sharp = geom_corners_are_smooth && fvar_corners_are_sharp;
        let sharpen_both_if_one_corner = fvar == FVarLinear::CornersPlus2;
        let sharpen_darts = sharpen_both_if_one_corner || self.has_linear_boundaries;

        let nv = level.vert_count;
        let mut vertex_mismatch = vec![false; nv as usize];
        self.vert_face_siblings = vec![0; level.vert_face_indices.len()];
        let mut total_value_count = 0i32;
        let mut v_values: Vec<Index> = Vec::new();
        let mut unique_values: Vec<Index> = Vec::new();
        let mut v_value_siblings: Vec<LocalIndex> = Vec::new();

        for v in 0..nv {
            let v_faces = level.vertex_faces(v);
            let v_in_face = level.vertex_face_local_indices(v);
            v_values.clear();
            for i in 0..v_faces.len() {
                v_values.push(
                    self.face_vert_values[(level.offset_of_face_vertices(v_faces[i])
                        + v_in_face[i] as i32) as usize],
                );
            }
            let v_edges = level.vertex_edges(v);
            let v_in_edge = level.vertex_edge_local_indices(v);
            let v_is_manifold = !level.vert_tags[v as usize].non_manifold();
            let v_is_boundary = level.vert_tags[v as usize].boundary();
            if v_is_manifold {
                for i in v_is_boundary as usize..v_faces.len() {
                    let next = i;
                    let prev = if i > 0 { i - 1 } else { v_faces.len() - 1 };
                    if v_values[next] != v_values[prev] {
                        let e = v_edges[i];
                        let ev = level.edge_vertices(e);
                        vertex_mismatch[ev[0] as usize] = true;
                        vertex_mismatch[ev[1] as usize] = true;
                        let t = &mut self.edge_tags[e as usize];
                        t.set_discts_v0(ev[0] == v);
                        t.set_discts_v1(ev[1] == v);
                        t.set_mismatch(true);
                        t.set_linear(self.has_linear_boundaries);
                    }
                }
            } else if !v_faces.is_empty() {
                for i in 0..v_edges.len() {
                    let e = v_edges[i];
                    let e_faces = level.edge_faces(e);
                    if e_faces.len() < 2 {
                        continue;
                    }
                    let e_in_face = level.edge_face_local_indices(e);
                    let ev = level.edge_vertices(e);
                    let vert_in_edge = v_in_edge[i] as i32;
                    let mut mark = false;
                    let mut value0 = 0;
                    for j in 0..e_faces.len() {
                        if mark {
                            break;
                        }
                        let f = e_faces[j];
                        let fv = level.face_vertices(f);
                        let fvals = self.face_values(level, f);
                        let edge_in_face = e_in_face[j] as i32;
                        let reversed = (ev[0] != fv[edge_in_face as usize]) as i32;
                        let mut vert_in_face = edge_in_face + ((vert_in_edge != reversed) as i32);
                        if vert_in_face == fv.len() as i32 {
                            vert_in_face = 0;
                        }
                        if j == 0 {
                            value0 = fvals[vert_in_face as usize];
                        } else {
                            mark = fvals[vert_in_face as usize] != value0;
                        }
                    }
                    if mark {
                        vertex_mismatch[ev[0] as usize] = true;
                        vertex_mismatch[ev[1] as usize] = true;
                        let t = &mut self.edge_tags[e as usize];
                        t.set_discts_v0(ev[0] == v);
                        t.set_discts_v1(ev[1] == v);
                        t.set_mismatch(true);
                        t.set_linear(self.has_linear_boundaries);
                    }
                }
            }
            if v_is_boundary && !vertex_mismatch[v as usize] {
                if self.has_linear_boundaries && !v_faces.is_empty() {
                    vertex_mismatch[v as usize] = true;
                    if v_is_manifold {
                        self.edge_tags[v_edges[0] as usize].set_linear(true);
                        self.edge_tags[v_edges[v_edges.len() - 1] as usize].set_linear(true);
                    } else {
                        for &e in v_edges {
                            if level.edge_tags[e as usize].boundary() {
                                self.edge_tags[e as usize].set_linear(true);
                            }
                        }
                    }
                } else if v_faces.len() == 1 && make_smooth_corners_sharp {
                    vertex_mismatch[v as usize] = true;
                }
            }

            let mut unique_count = 1usize;
            unique_values.clear();
            v_value_siblings.clear();
            unique_values.push(v_values.first().copied().unwrap_or(0));
            v_value_siblings.push(0);
            for i in 1..v_faces.len() {
                if v_values[i] == v_values[i - 1] {
                    let s = v_value_siblings[i - 1];
                    v_value_siblings.push(s);
                } else {
                    let mut s = unique_count as LocalIndex;
                    if unique_count == 1 || (unique_count == 2 && unique_values[0] != v_values[i]) {
                        unique_values.push(v_values[i]);
                        unique_count += 1;
                    } else if let Some(p) = unique_values.iter().position(|&x| x == v_values[i]) {
                        s = p as LocalIndex;
                    } else {
                        unique_values.push(v_values[i]);
                        unique_count += 1;
                    }
                    v_value_siblings.push(s);
                }
            }
            if !v_is_manifold && !vertex_mismatch[v as usize] {
                vertex_mismatch[v as usize] = unique_count > 1;
            }
            self.vert_sibling_counts[v as usize] = unique_count as LocalIndex;
            self.vert_sibling_offsets[v as usize] = total_value_count;
            total_value_count += unique_count as i32;
            if unique_count > 1 {
                let o = level.offset_of_vertex_faces(v) as usize;
                let n = v_faces.len();
                self.vert_face_siblings[o..o + n].copy_from_slice(&v_value_siblings[..n]);
            }
        }

        self.resize_vertex_values(total_value_count as usize);

        for v in 0..nv {
            let v_faces = level.vertex_faces(v);
            let v_in_face = level.vertex_face_local_indices(v);
            let off = self.vert_sibling_offsets[v as usize] as usize;
            let n_values = self.vert_sibling_counts[v as usize] as usize;
            self.vert_value_indices[off] = if !v_faces.is_empty() {
                self.face_vert_values
                    [(level.offset_of_face_vertices(v_faces[0]) + v_in_face[0] as i32) as usize]
            } else {
                0
            };
            if !vertex_mismatch[v as usize] {
                continue;
            }
            if n_values > 1 {
                let so = level.offset_of_vertex_faces(v) as usize;
                let mut next_sibling = 1usize;
                for i in 1..v_faces.len() {
                    if self.vert_face_siblings[so + i] as usize == next_sibling {
                        self.vert_value_indices[off + next_sibling] = self.face_vert_values[(level
                            .offset_of_face_vertices(v_faces[i])
                            + v_in_face[i] as i32)
                            as usize];
                        next_sibling += 1;
                    }
                }
            }
            let vtag = level.vert_tags[v as usize];
            let mut all_corners_sharp = self.has_linear_boundaries
                || vtag.inf_sharp()
                || vtag.non_manifold()
                || (self.has_dependent_sharpness && n_values > 2)
                || (sharpen_darts && n_values == 1 && !vtag.boundary());

            let mut spans = vec![ValueSpan::default(); n_values];
            self.gather_value_spans(level, v, &mut spans);

            let mut has_dependent_values_to_sharpen = false;
            if !all_corners_sharp && self.has_dependent_sharpness && n_values == 2 {
                all_corners_sharp = spans[0].inf_sharp_edge_count > 0
                    || spans[1].inf_sharp_edge_count > 0
                    || spans[0].discts_edge_count > 0
                    || spans[1].discts_edge_count > 0;
                if sharpen_both_if_one_corner {
                    all_corners_sharp |= spans[0].size == 1 || spans[1].size == 1;
                }
                has_dependent_values_to_sharpen =
                    (spans[0].semi_sharp_edge_count > 0) != (spans[1].semi_sharp_edge_count > 0);
            }

            for i in 0..n_values {
                let mut tag = ValueTag::default();
                tag.set_mismatch(true);
                let span = spans[i];
                if span.discts_edge_count > 0 {
                    tag.set_non_manifold(true);
                    self.vert_value_tags[off + i] = tag;
                    continue;
                }
                let is_inf_sharp = all_corners_sharp
                    || span.inf_sharp_edge_count > 0
                    || (span.size == 1 && fvar_corners_are_sharp);
                if span.size == 1 {
                    tag.set_xordinary(!is_inf_sharp);
                } else {
                    tag.set_xordinary(span.size as i32 != regular_boundary_valence);
                }
                tag.set_inf_sharp_edges(span.inf_sharp_edge_count > 0);
                let inf_irregular = if span.inf_sharp_edge_count > 0 {
                    (span.size as i32 - span.inf_sharp_edge_count as i32) > 1
                } else if is_inf_sharp {
                    span.size > 1
                } else {
                    tag.xordinary()
                };
                tag.set_inf_irregular(inf_irregular);
                if !is_inf_sharp {
                    if span.semi_sharp_edge_count > 0 || vtag.semi_sharp() {
                        tag.set_semi_sharp(true);
                    } else if has_dependent_values_to_sharpen {
                        tag.set_semi_sharp(true);
                        tag.set_dep_sharp(true);
                    } else {
                        tag.set_crease(true);
                    }
                    if self.has_smooth_boundaries() {
                        let start = span.start;
                        let end = if i == 0 && span.start != 0 {
                            (span.start as i32 + span.size as i32 - 1 - v_faces.len() as i32)
                                as LocalIndex
                        } else {
                            (span.start as i32 + span.size as i32 - 1) as LocalIndex
                        };
                        self.vert_value_crease_ends[off + i] = CreaseEndPair {
                            start_face: start,
                            end_face: end,
                        };
                    }
                }
                self.vert_value_tags[off + i] = tag;
            }
        }
    }

    /// `gatherValueSpans`
    fn gather_value_spans(&self, level: &Level, v: Index, spans: &mut [ValueSpan]) {
        let v_edges = level.vertex_edges(v);
        let v_faces = level.vertex_faces(v);
        let sib = self.vertex_face_siblings(level, v);
        let single = self.num_vertex_values(v) == 1;
        let is_boundary = v_edges.len() > v_faces.len();
        let non_manifold = level.vert_tags[v as usize].non_manifold();
        if non_manifold {
            for s in spans.iter_mut() {
                s.size = 0;
                s.discts_edge_count = 1;
            }
        } else if single && !is_boundary {
            spans[0].size = 0;
            spans[0].start = 0;
            for i in 0..v_edges.len() {
                let e = v_edges[i];
                if self.edge_tags[e as usize].mismatch() {
                    if spans[0].size > 0 {
                        spans[0].discts_edge_count = 1;
                        break;
                    } else {
                        spans[0].size = v_faces.len() as LocalIndex;
                        spans[0].start = i as LocalIndex;
                    }
                } else if level.edge_tags[e as usize].inf_sharp() {
                    spans[0].inf_sharp_edge_count += 1;
                } else if level.edge_tags[e as usize].semi_sharp() {
                    spans[0].semi_sharp_edge_count += 1;
                }
            }
            spans[0].size = v_faces.len() as LocalIndex;
        } else {
            spans[0].size = 1;
            spans[0].start = 0;
            let nf = v_faces.len();
            if !is_boundary && sib[nf - 1] == 0 {
                let e = v_edges[0];
                if self.edge_tags[e as usize].mismatch() {
                    spans[0].discts_edge_count += 1;
                } else if level.edge_tags[e as usize].inf_sharp() {
                    spans[0].inf_sharp_edge_count += 1;
                } else if level.edge_tags[e as usize].semi_sharp() {
                    spans[0].semi_sharp_edge_count += 1;
                }
            }
            for i in 1..nf {
                let s = sib[i] as usize;
                if sib[i] == sib[i - 1] {
                    let e = v_edges[i];
                    if self.edge_tags[e as usize].mismatch() {
                        spans[s].discts_edge_count += 1;
                    } else if level.edge_tags[e as usize].inf_sharp() {
                        spans[s].inf_sharp_edge_count += 1;
                    } else if level.edge_tags[e as usize].semi_sharp() {
                        spans[s].semi_sharp_edge_count += 1;
                    }
                } else {
                    if spans[s].size > 0 {
                        spans[s].discts_edge_count += 1;
                    }
                    spans[s].start = i as LocalIndex;
                }
                spans[s].size += 1;
            }
            if sib[nf - 1] == 0 && !is_boundary {
                spans[0].discts_edge_count = spans[0].discts_edge_count.wrapping_sub(1);
            }
        }
    }

    /// `getVertexCreaseEndValues`
    pub(crate) fn vertex_crease_end_values(
        &self,
        level: &Level,
        v: Index,
        sibling: usize,
    ) -> [Index; 2] {
        let ends = self.vertex_value_crease_ends(v)[sibling];
        let v_faces = level.vertex_faces(v);
        let v_in_face = level.vertex_face_local_indices(v);
        let f0 = ends.start_face as usize;
        let f1 = ends.end_face as usize;
        let face0 = self.face_values(level, v_faces[f0]);
        let face1 = self.face_values(level, v_faces[f1]);
        let mut e0 = v_in_face[f0] as usize;
        let mut e1 = v_in_face[f1] as usize;
        e0 = if e0 == face0.len() - 1 { 0 } else { e0 + 1 };
        e1 = (if e1 > 0 { e1 } else { face1.len() }) - 1;
        [face0[e0], face1[e1]]
    }

    /// `getEdgeFaceValues`
    pub(crate) fn edge_face_values(
        &self,
        level: &Level,
        e: Index,
        face_in_edge: usize,
    ) -> [Index; 2] {
        let ev = level.edge_vertices(e);
        if self.num_vertex_values(ev[0]) + self.num_vertex_values(ev[1]) > 2 {
            let f = level.edge_faces(e)[face_in_edge];
            let k = level.edge_face_local_indices(e)[face_in_edge] as usize;
            let fv = self.face_values(level, f);
            let mut out = [fv[k], fv[if k + 1 < fv.len() { k + 1 } else { 0 }]];
            if ev[0] != level.face_vertices(f)[k] {
                out.swap(0, 1);
            }
            out
        } else if level.depth > 0 {
            [
                self.vertex_value_offset(ev[0], 0),
                self.vertex_value_offset(ev[1], 0),
            ]
        } else {
            [self.vertex_value(ev[0], 0), self.vertex_value(ev[1], 0)]
        }
    }

    /// `getVertexEdgeValues`
    pub(crate) fn vertex_edge_values(&self, level: &Level, v: Index, out: &mut Vec<Index>) {
        out.clear();
        let v_edges = level.vertex_edges(v);
        let v_in_edge = level.vertex_edge_local_indices(v);
        let v_faces = level.vertex_faces(v);
        let v_in_face = level.vertex_face_local_indices(v);
        let is_boundary = level.vert_tags[v as usize].boundary();
        let is_manifold = !level.vert_tags[v as usize].non_manifold();
        let base = level.depth == 0;
        for i in 0..v_edges.len() {
            let e = v_edges[i];
            let ev = level.edge_vertices(e);
            let other = ev[(v_in_edge[i] == 0) as usize];
            let value = if self.num_vertex_values(other) == 1 {
                if base {
                    self.vertex_value(other, 0)
                } else {
                    self.vertex_value_offset(other, 0)
                }
            } else if is_manifold {
                if is_boundary && i == v_edges.len() - 1 {
                    let fv = self.face_values(level, v_faces[i - 1]);
                    let k = v_in_face[i - 1] as usize;
                    fv[if k > 0 { k - 1 } else { fv.len() - 1 }]
                } else {
                    let fv = self.face_values(level, v_faces[i]);
                    let k = v_in_face[i] as usize;
                    fv[if k == fv.len() - 1 { 0 } else { k + 1 }]
                }
            } else {
                let f0 = level.edge_faces(e)[0];
                let k = level.edge_face_local_indices(e)[0] as usize;
                let fverts = level.face_vertices(f0);
                let fv = self.face_values(level, f0);
                if other == fverts[k] {
                    fv[k]
                } else {
                    fv[if k == fv.len() - 1 { 0 } else { k + 1 }]
                }
            };
            out.push(value);
        }
    }

    /// `initializeFaceValuesFromFaceVertices`
    pub(crate) fn initialize_face_values_from_face_vertices(&mut self, level: &Level) {
        self.face_vert_values
            .copy_from_slice(&level.face_vert_indices);
    }

    /// `initializeFaceValuesFromVertexFaceSiblings`
    pub(crate) fn initialize_face_values_from_vertex_face_siblings(&mut self, level: &Level) {
        for (i, &v) in level.face_vert_indices.iter().enumerate() {
            self.face_vert_values[i] = self.vertex_value_offset(v, 0);
        }
        for v in 0..level.vert_count {
            if self.num_vertex_values(v) > 1 {
                let v_faces = level.vertex_faces(v);
                let v_in_face = level.vertex_face_local_indices(v);
                let so = level.offset_of_vertex_faces(v) as usize;
                for j in 0..v_faces.len() {
                    let s = self.vert_face_siblings[so + j];
                    if s != 0 {
                        let o = level.offset_of_face_vertices(v_faces[j]) as usize;
                        self.face_vert_values[o + v_in_face[j] as usize] += s as Index;
                    }
                }
            }
        }
    }
}
