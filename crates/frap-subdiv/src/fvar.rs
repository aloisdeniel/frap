// SPDX-License-Identifier: GPL-3.0-or-later
//! Face-varying channels (`Vtr::internal::FVarLevel`): the values of a UV map around each
//! vertex ("siblings"), their topological classification (matching the vertex topology, or a
//! corner / crease / semi-sharp value of a UV seam or boundary) under the channel's
//! `FVarLinearInterpolation`, and the queries refinement and limit masks need.
//!
//! Ported from OpenSubdiv 3.x `opensubdiv/vtr/fvarLevel.{h,cpp}`. Value indices are global
//! "slots": every value belongs to exactly one vertex (Blender's converter numbers UV values
//! per vertex, `subdiv_converter_mesh.cc` `precalc_uv_layer`), so the slot is used both as the
//! value and as the vertex-value index OpenSubdiv keeps apart at level 0. Child levels are
//! classified by running the same analysis on the refined face-values: child values inherit
//! the topology of their parents, so the analysis reproduces `FVarRefinement`'s propagated
//! tags.

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use smallvec::SmallVec;

use crate::FVarLinear;
use crate::vtr::Level;

/// `FVarLevel::ETag`
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct FETag {
    pub mismatch: bool,
    pub linear: bool,
}

/// `FVarLevel::ValueTag`
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct ValueTag {
    pub mismatch: bool,
    pub non_manifold: bool,
    pub crease: bool,
    pub semi_sharp: bool,
    pub dep_sharp: bool,
}

impl ValueTag {
    pub fn is_corner(self) -> bool {
        !self.crease
    }
}

/// `FVarLevel::ValueSpan`
#[derive(Copy, Clone, Debug, Default)]
struct ValueSpan {
    size: u16,
    start: u16,
    discts_edge_count: u16,
    semi_sharp_edge_count: u16,
    inf_sharp_edge_count: u16,
}

/// One face-varying channel on one level.
#[derive(Clone, Debug, Default)]
pub(crate) struct FVarLevel {
    pub is_linear: bool,
    pub has_linear_boundaries: bool,
    /// Value slot of every face-corner (aligned with `Level::face_verts`).
    pub face_values: Vec<u32>,
    pub edge_tags: Vec<FETag>,
    /// Distinct value slots around each vertex, in sibling order.
    pub vert_values: Vec<SmallVec<[u32; 2]>>,
    /// Sibling index of each incident face of each vertex.
    pub vert_face_siblings: Vec<SmallVec<[u16; 6]>>,
    /// Per slot.
    pub value_tags: Vec<ValueTag>,
    /// Per slot: `[start face, end face]` (local indices around the vertex) of a crease.
    pub crease_ends: Vec<[u16; 2]>,
}

impl FVarLevel {
    pub fn has_crease_ends(&self) -> bool {
        !self.has_linear_boundaries
    }

    /// Whether the vertex's values match the vertex topology (`valueTopologyMatches` of its
    /// first value).
    pub fn vertex_matches(&self, v: usize) -> bool {
        !self.value_tags[self.vert_values[v][0] as usize].mismatch
    }

    pub fn face_values(&self, lvl: &Level, f: usize) -> &[u32] {
        &self.face_values[lvl.face_range(f)]
    }

    /// `FVarLevel::completeTopologyFromFaceValues` (with `regularBoundaryValence == 2`).
    /// `nslots` is the number of value slots referenced by `face_values`.
    pub fn complete(
        lvl: &Level,
        face_values: Vec<u32>,
        nslots: usize,
        fvar: FVarLinear,
        geom_corners_sharp: bool,
    ) -> FVarLevel {
        let is_linear = fvar == FVarLinear::All;
        let has_linear_boundaries = matches!(fvar, FVarLinear::All | FVarLinear::Boundaries);
        let has_dependent_sharpness = matches!(
            fvar,
            FVarLinear::CornersPlusOne | FVarLinear::CornersPlusTwo
        );
        let fvar_corners_sharp = fvar != FVarLinear::None;
        let make_smooth_corners_sharp = !geom_corners_sharp && fvar_corners_sharp;
        let sharpen_both_if_one_corner = fvar == FVarLinear::CornersPlusTwo;
        let sharpen_darts = sharpen_both_if_one_corner || has_linear_boundaries;

        let nv = lvl.nverts;
        let mut fl = FVarLevel {
            is_linear,
            has_linear_boundaries,
            face_values,
            edge_tags: vec![FETag::default(); lvl.nedges()],
            vert_values: vec![SmallVec::new(); nv],
            vert_face_siblings: vec![SmallVec::new(); nv],
            value_tags: vec![ValueTag::default(); nslots],
            crease_ends: vec![[0, 0]; nslots],
        };
        let mut mismatch = vec![false; nv];
        let regular_boundary_valence = 2usize;
        for v in 0..nv {
            let vfaces = &lvl.vert_faces[v];
            let vvals: SmallVec<[u32; 6]> = vfaces
                .iter()
                .enumerate()
                .map(|(i, &f)| {
                    fl.face_values
                        [lvl.face_offsets[f as usize] as usize + lvl.vert_face_local[v][i] as usize]
                })
                .collect();
            let vedges = &lvl.vert_edges[v];
            let manifold = !lvl.vert_tags[v].non_manifold;
            let boundary = lvl.vert_tags[v].boundary;
            if manifold {
                for i in (boundary as usize)..vfaces.len() {
                    let prev = if i > 0 { i - 1 } else { vfaces.len() - 1 };
                    if vvals[i] != vvals[prev] {
                        let e = vedges[i] as usize;
                        let ev = lvl.edge_verts[e];
                        mismatch[ev[0] as usize] = true;
                        mismatch[ev[1] as usize] = true;
                        fl.edge_tags[e].mismatch = true;
                        fl.edge_tags[e].linear = has_linear_boundaries;
                    }
                }
            } else if !vfaces.is_empty() {
                for (i, &e) in vedges.iter().enumerate() {
                    let e = e as usize;
                    let efaces = &lvl.edge_faces[e];
                    if efaces.len() < 2 {
                        continue;
                    }
                    let ev = lvl.edge_verts[e];
                    let vert_in_edge = lvl.vert_edge_local[v][i] as usize;
                    let mut first = 0u32;
                    let mut discts = false;
                    for (j, &f) in efaces.iter().enumerate() {
                        let fverts = lvl.face_verts(f as usize);
                        let fedges = lvl.face_edges(f as usize);
                        let edge_in_face =
                            fedges.iter().position(|&x| x as usize == e).unwrap_or(0);
                        let reversed = (ev[0] != fverts[edge_in_face]) as usize;
                        let mut vif = edge_in_face + (vert_in_edge != reversed) as usize;
                        if vif == fverts.len() {
                            vif = 0;
                        }
                        let val = fl.face_values(lvl, f as usize)[vif];
                        if j == 0 {
                            first = val;
                        } else if val != first {
                            discts = true;
                            break;
                        }
                    }
                    if discts {
                        mismatch[ev[0] as usize] = true;
                        mismatch[ev[1] as usize] = true;
                        fl.edge_tags[e].mismatch = true;
                        fl.edge_tags[e].linear = has_linear_boundaries;
                    }
                }
            }
            if boundary && !mismatch[v] {
                if has_linear_boundaries && !vfaces.is_empty() {
                    mismatch[v] = true;
                    if manifold {
                        let first = vedges[0] as usize;
                        let last = vedges[vedges.len() - 1] as usize;
                        fl.edge_tags[first].linear = true;
                        fl.edge_tags[last].linear = true;
                    } else {
                        for &e in vedges {
                            if lvl.edge_tags[e as usize].boundary {
                                fl.edge_tags[e as usize].linear = true;
                            }
                        }
                    }
                } else if vfaces.len() == 1 && make_smooth_corners_sharp {
                    mismatch[v] = true;
                }
            }
            // Unique values in order of appearance, and the sibling of each face.
            let mut uniq: SmallVec<[u32; 2]> = SmallVec::new();
            let mut sib: SmallVec<[u16; 6]> = SmallVec::new();
            for (i, &val) in vvals.iter().enumerate() {
                if i > 0 && val == vvals[i - 1] {
                    let s = sib[i - 1];
                    sib.push(s);
                    continue;
                }
                match uniq.iter().position(|&u| u == val) {
                    Some(p) => sib.push(p as u16),
                    None => {
                        sib.push(uniq.len() as u16);
                        uniq.push(val);
                    }
                }
            }
            if uniq.is_empty() {
                uniq.push(0);
            }
            if !manifold && !mismatch[v] {
                mismatch[v] = uniq.len() > 1;
            }
            fl.vert_values[v] = uniq;
            fl.vert_face_siblings[v] = sib;
        }

        for v in 0..nv {
            if !mismatch[v] {
                continue;
            }
            let nvals = fl.vert_values[v].len();
            let vtag = lvl.vert_tags[v];
            let mut all_corners_sharp = has_linear_boundaries
                || vtag.inf_sharp
                || vtag.non_manifold
                || (has_dependent_sharpness && nvals > 2)
                || (sharpen_darts && nvals == 1 && !vtag.boundary);
            let spans = fl.gather_value_spans(lvl, v);
            let mut has_dependent_to_sharpen = false;
            if !all_corners_sharp && has_dependent_sharpness && nvals == 2 {
                all_corners_sharp = spans[0].inf_sharp_edge_count > 0
                    || spans[1].inf_sharp_edge_count > 0
                    || spans[0].discts_edge_count > 0
                    || spans[1].discts_edge_count > 0;
                if sharpen_both_if_one_corner {
                    all_corners_sharp |= spans[0].size == 1 || spans[1].size == 1;
                }
                has_dependent_to_sharpen =
                    (spans[0].semi_sharp_edge_count > 0) != (spans[1].semi_sharp_edge_count > 0);
            }
            let nfaces = lvl.vert_faces[v].len();
            for i in 0..nvals {
                let slot = fl.vert_values[v][i] as usize;
                let span = spans[i];
                let mut tag = ValueTag {
                    mismatch: true,
                    ..ValueTag::default()
                };
                if span.discts_edge_count > 0 {
                    tag.non_manifold = true;
                    fl.value_tags[slot] = tag;
                    continue;
                }
                let inf_sharp = all_corners_sharp
                    || span.inf_sharp_edge_count > 0
                    || (span.size == 1 && fvar_corners_sharp);
                let _ = regular_boundary_valence;
                if !inf_sharp {
                    if span.semi_sharp_edge_count > 0 || vtag.semi_sharp {
                        tag.semi_sharp = true;
                    } else if has_dependent_to_sharpen {
                        tag.semi_sharp = true;
                        tag.dep_sharp = true;
                    } else {
                        tag.crease = true;
                    }
                    if fl.has_crease_ends() {
                        let start = span.start;
                        let end = if i == 0 && span.start != 0 {
                            (span.start as i32 + span.size as i32 - 1 - nfaces as i32) as u16
                        } else {
                            span.start + span.size - 1
                        };
                        fl.crease_ends[slot] = [start, end];
                    }
                }
                fl.value_tags[slot] = tag;
            }
        }
        fl
    }

    /// `FVarLevel::gatherValueSpans`
    fn gather_value_spans(&self, lvl: &Level, v: usize) -> SmallVec<[ValueSpan; 2]> {
        let vedges = &lvl.vert_edges[v];
        let vfaces = &lvl.vert_faces[v];
        let sib = &self.vert_face_siblings[v];
        let nvals = self.vert_values[v].len();
        let mut spans: SmallVec<[ValueSpan; 2]> = SmallVec::from_elem(ValueSpan::default(), nvals);
        let single = nvals == 1;
        let is_boundary = vedges.len() > vfaces.len();
        let etag = |e: u32| lvl.edge_tags[e as usize];
        if lvl.vert_tags[v].non_manifold {
            for s in spans.iter_mut() {
                s.size = 0;
                s.discts_edge_count = 1;
            }
        } else if single && !is_boundary {
            spans[0].size = 0;
            spans[0].start = 0;
            for (i, &e) in vedges.iter().enumerate() {
                if self.edge_tags[e as usize].mismatch {
                    if spans[0].size > 0 {
                        spans[0].discts_edge_count = 1;
                        break;
                    } else {
                        spans[0].size = vfaces.len() as u16;
                        spans[0].start = i as u16;
                    }
                } else if etag(e).inf_sharp {
                    spans[0].inf_sharp_edge_count += 1;
                } else if etag(e).semi_sharp {
                    spans[0].semi_sharp_edge_count += 1;
                }
            }
            spans[0].size = vfaces.len() as u16;
        } else {
            spans[0].size = 1;
            spans[0].start = 0;
            let n = vfaces.len();
            if !is_boundary && sib[n - 1] == 0 {
                let e = vedges[0];
                if self.edge_tags[e as usize].mismatch {
                    spans[0].discts_edge_count += 1;
                } else if etag(e).inf_sharp {
                    spans[0].inf_sharp_edge_count += 1;
                } else if etag(e).semi_sharp {
                    spans[0].semi_sharp_edge_count += 1;
                }
            }
            for i in 1..n {
                let s = sib[i] as usize;
                if sib[i] == sib[i - 1] {
                    let e = vedges[i];
                    if self.edge_tags[e as usize].mismatch {
                        spans[s].discts_edge_count += 1;
                    } else if etag(e).inf_sharp {
                        spans[s].inf_sharp_edge_count += 1;
                    } else if etag(e).semi_sharp {
                        spans[s].semi_sharp_edge_count += 1;
                    }
                } else {
                    if spans[s].size > 0 {
                        spans[s].discts_edge_count += 1;
                    }
                    spans[s].start = i as u16;
                }
                spans[s].size += 1;
            }
            if sib[n - 1] == 0 && !is_boundary {
                spans[0].discts_edge_count = spans[0].discts_edge_count.saturating_sub(1);
            }
        }
        spans
    }

    /// `FVarLevel::getEdgeFaceValues`: the values at the two ends of `e` within its incident
    /// face `face_in_edge`, in edge-vertex order.
    pub fn edge_face_values(&self, lvl: &Level, e: usize, face_in_edge: usize) -> [u32; 2] {
        let ev = lvl.edge_verts[e];
        if self.vert_values[ev[0] as usize].len() + self.vert_values[ev[1] as usize].len() > 2 {
            let f = lvl.edge_faces[e][face_in_edge] as usize;
            let fe = lvl.face_edges(f);
            let k = fe.iter().position(|&x| x as usize == e).unwrap_or(0);
            let fvals = self.face_values(lvl, f);
            let n = fvals.len();
            let mut out = [fvals[k], fvals[if k + 1 < n { k + 1 } else { 0 }]];
            if ev[0] != lvl.face_verts(f)[k] {
                out.swap(0, 1);
            }
            out
        } else {
            [
                self.vert_values[ev[0] as usize][0],
                self.vert_values[ev[1] as usize][0],
            ]
        }
    }

    /// `FVarLevel::getVertexEdgeValues`: the value at the other end of every incident edge
    /// (for a vertex whose values match the vertex topology).
    pub fn vertex_edge_values(&self, lvl: &Level, v: usize) -> SmallVec<[u32; 6]> {
        let vedges = &lvl.vert_edges[v];
        let vfaces = &lvl.vert_faces[v];
        let boundary = lvl.vert_tags[v].boundary;
        let manifold = !lvl.vert_tags[v].non_manifold;
        let mut out = SmallVec::new();
        for (i, &e) in vedges.iter().enumerate() {
            let other = lvl.other_vert(e, v as u32) as usize;
            if self.vert_values[other].len() == 1 {
                out.push(self.vert_values[other][0]);
            } else if manifold {
                if boundary && i == vedges.len() - 1 {
                    let f = vfaces[i - 1] as usize;
                    let fvals = self.face_values(lvl, f);
                    let k = lvl.vert_face_local[v][i - 1] as usize;
                    let prev = if k > 0 { k - 1 } else { fvals.len() - 1 };
                    out.push(fvals[prev]);
                } else {
                    let f = vfaces[i] as usize;
                    let fvals = self.face_values(lvl, f);
                    let k = lvl.vert_face_local[v][i] as usize;
                    let next = if k + 1 == fvals.len() { 0 } else { k + 1 };
                    out.push(fvals[next]);
                }
            } else {
                let f0 = lvl.edge_faces[e as usize][0] as usize;
                let fe = lvl.face_edges(f0);
                let k = fe.iter().position(|&x| x == e).unwrap_or(0);
                let fverts = lvl.face_verts(f0);
                let fvals = self.face_values(lvl, f0);
                if other as u32 == fverts[k] {
                    out.push(fvals[k]);
                } else {
                    out.push(fvals[if k + 1 == fvals.len() { 0 } else { k + 1 }]);
                }
            }
        }
        out
    }

    /// `FVarLevel::getVertexCreaseEndValues`
    pub fn crease_end_values(&self, lvl: &Level, v: usize, slot: u32) -> [u32; 2] {
        let [f0, f1] = self.crease_ends[slot as usize];
        let vfaces = &lvl.vert_faces[v];
        let face0 = vfaces[f0 as usize] as usize;
        let face1 = vfaces[f1 as usize] as usize;
        let v0 = self.face_values(lvl, face0);
        let v1 = self.face_values(lvl, face1);
        let mut end0 = lvl.vert_face_local[v][f0 as usize] as usize;
        let mut end1 = lvl.vert_face_local[v][f1 as usize] as usize;
        end0 = if end0 == v0.len() - 1 { 0 } else { end0 + 1 };
        end1 = if end1 > 0 { end1 } else { v1.len() } - 1;
        [v0[end0], v1[end1]]
    }

    /// Interior edges (local indices around `v`) of the span of a crease value: the edges
    /// between its start and end faces.
    pub fn crease_interior_edges(&self, lvl: &Level, v: usize, slot: u32) -> SmallVec<[usize; 6]> {
        let [s, e] = self.crease_ends[slot as usize];
        let n = lvl.vert_edges[v].len();
        let mut out = SmallVec::new();
        if e > s {
            out.extend((s as usize + 1)..=(e as usize));
        } else if s > e {
            out.extend((s as usize + 1)..n);
            out.extend(0..=(e as usize));
        }
        out
    }
}
