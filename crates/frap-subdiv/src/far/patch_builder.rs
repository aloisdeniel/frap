// SPDX-License-Identifier: GPL-3.0-or-later
//! `Far::PatchBuilder` / `CatmarkPatchBuilder` with the options `PatchTableFactory` gives it
//! for Blender (`BASIS_REGULAR`, `BASIS_GREGORY` end caps, `fillMissingBoundaryPoints`,
//! `useInfSharpPatch`, legacy sharp corner patches): which faces are patches, whether a patch
//! (or its face-varying counterpart) is regular, the regular patch points and boundary mask,
//! the irregular corner spans and source points, and the patch parameterisation.
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/far/{patchBuilder,catmarkPatchBuilder,
//! patchParam,ptexIndices}.{h,cpp}` (Pixar, TOST / Apache-2.0 modified).

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::fvar_level::FVarLevel;
use super::gregory::{self, SourcePatch, SparseMatrix};
use super::level::{Level, VSpan, fast_mod4};
use super::refiner::TopologyRefiner;
use super::tags::{ETag, RULE_DART, VTag};
use super::{INDEX_INVALID, Index, LocalIndex};

/// `Far::PatchParam`, unpacked.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PatchParam {
    pub face_id: i32,
    pub u: i32,
    pub v: i32,
    pub depth: i32,
    pub non_quad_root: bool,
    pub boundary: i32,
    pub transition: i32,
    pub regular: bool,
}

/// `Far::PtexIndices`: the first ptex face of every base face.
pub(crate) fn ptex_indices(base: &Level) -> Vec<i32> {
    let mut out = Vec::with_capacity(base.face_count as usize);
    let mut n = 0;
    for f in 0..base.face_count {
        out.push(n);
        let k = base.num_face_vertices(f);
        n += if k == 4 { 1 } else { k };
    }
    out
}

fn fast_mod_n(x: usize, n: usize) -> usize {
    if x < n { x } else { x - n }
}

/// `getSingularEdgeMask`
fn singular_edge_mask(include_all_inf_sharp: bool) -> ETag {
    let mut t = ETag::default();
    t.set_boundary(true);
    t.set_non_manifold(true);
    t.set_inf_sharp(include_all_inf_sharp);
    t
}

/// `isEdgeSingular`
fn is_edge_singular(level: &Level, fvar: Option<&FVarLevel>, e: Index, mask: ETag) -> bool {
    let mut t = level.edge_tag(e);
    if let Some(fv) = fvar {
        t = fv.edge_tags[e as usize].combine_with_level_etag(t);
    }
    t.0 & mask.0 != 0
}

/// `identifyManifoldCornerSpan`
fn identify_manifold_corner_span(
    level: &Level,
    f: Index,
    corner: usize,
    mask: ETag,
    fvc: Option<usize>,
) -> VSpan {
    let fvar = fvc.map(|c| level.fvar_level(c));
    let fv = level.face_vertices(f);
    let fe = level.face_edges(f);
    let v_edges = level.vertex_edges(fv[corner]);
    let n = v_edges.len();
    let lead_start = v_edges.iter().position(|&e| e == fe[corner]).unwrap_or(n);
    let trail_start = fast_mod_n(lead_start + 1, n);
    let mut span = VSpan {
        num_faces: 1,
        corner_in_span: 0,
        ..VSpan::default()
    };
    let mut lead = lead_start;
    while !is_edge_singular(level, fvar, v_edges[lead], mask) {
        span.num_faces += 1;
        span.corner_in_span += 1;
        lead = fast_mod_n(lead + n - 1, n);
        if lead == trail_start {
            break;
        }
    }
    let mut trail = trail_start;
    if trail != lead {
        while !is_edge_singular(level, fvar, v_edges[trail], mask) {
            span.num_faces += 1;
            trail = fast_mod_n(trail + 1, n);
            if trail == lead_start {
                break;
            }
        }
    }
    span.start_face = lead as LocalIndex;
    span
}

/// `identifyNonManifoldCornerSpan`
fn identify_non_manifold_corner_span(
    level: &Level,
    f: Index,
    corner: usize,
    mask: ETag,
    fvc: Option<usize>,
) -> VSpan {
    let fvar = fvc.map(|c| level.fvar_level(c));
    let mut fe = level.face_edges(f);
    let e_lead_start = fe[corner];
    let e_trail_start = fe[(corner + fe.len() - 1) % fe.len()];
    let mut span = VSpan {
        num_faces: 1,
        corner_in_span: 0,
        ..VSpan::default()
    };
    let mut start_face = f;
    let mut start_corner = corner;
    let mut f_lead = f;
    let mut e_lead = e_lead_start;
    while !is_edge_singular(level, fvar, e_lead, mask) {
        span.num_faces += 1;
        span.corner_in_span += 1;
        let ef = level.edge_faces(e_lead);
        f_lead = if ef[0] == f_lead { ef[1] } else { ef[0] };
        fe = level.face_edges(f_lead);
        start_face = f_lead;
        start_corner = (fe.iter().position(|&x| x == e_lead).unwrap_or(0) + 1) % fe.len();
        e_lead = fe[start_corner];
        if e_lead == e_trail_start {
            span.periodic = !is_edge_singular(level, fvar, e_lead, mask);
            break;
        }
    }
    let mut f_trail = f;
    let mut e_trail = e_trail_start;
    if e_trail != e_lead {
        while !is_edge_singular(level, fvar, e_trail, mask) {
            span.num_faces += 1;
            let ef = level.edge_faces(e_trail);
            f_trail = if ef[0] == f_trail { ef[1] } else { ef[0] };
            let fe2 = level.face_edges(f_trail);
            let k = fe2.iter().position(|&x| x == e_trail).unwrap_or(0);
            e_trail = fe2[(k + fe2.len() - 1) % fe2.len()];
            if e_trail == e_lead_start {
                span.periodic = !is_edge_singular(level, fvar, e_trail, mask);
                break;
            }
        }
    }
    let v = level.face_vertices(f)[corner];
    let vf = level.vertex_faces(v);
    let vin = level.vertex_face_local_indices(v);
    span.start_face = vf.len() as LocalIndex;
    for i in 0..vf.len() {
        if vf[i] == start_face && vin[i] as usize == start_corner {
            span.start_face = i as LocalIndex;
            break;
        }
    }
    span
}

/// `gatherRegularPartialRingAroundVertex` (quads).
fn gather_regular_partial_ring(
    level: &Level,
    v: Index,
    span: &VSpan,
    ring: &mut Vec<Index>,
    fvc: Option<usize>,
) -> usize {
    ring.clear();
    let manifold = !level.vertex_tag(v).non_manifold();
    let vf = level.vertex_faces(v);
    let vin = level.vertex_face_local_indices(v);
    let n_faces = span.num_faces as usize;
    let start = span.start_face as usize;
    let mut next_face = vf[start];
    let mut v_in_next = vin[start] as usize;
    for i in 0..n_faces {
        let this_face = next_face;
        let v_in_this = v_in_next;
        let fp = face_points(level, this_face, fvc);
        let is_quad = fp.len() == 4;
        if is_quad {
            ring.push(fp[fast_mod4(v_in_this as i32 + 1) as usize]);
            ring.push(fp[fast_mod4(v_in_this as i32 + 2) as usize]);
        } else {
            ring.push(fp[(v_in_this + 1) % 3]);
        }
        if i == n_faces - 1 {
            if !span.periodic {
                if is_quad {
                    ring.push(fp[fast_mod4(v_in_this as i32 + 3) as usize]);
                } else {
                    ring.push(fp[(v_in_this + 2) % 3]);
                }
            }
        } else if manifold {
            let k = fast_mod_n(start + i + 1, vf.len());
            next_face = vf[k];
            v_in_next = vin[k] as usize;
        } else {
            let n = fp.len();
            let next_in_this = fast_mod_n(v_in_this + n - 1, n);
            let next_edge = level.face_edges(this_face)[next_in_this];
            let ef = level.edge_faces(next_edge);
            next_face = if ef[0] == this_face { ef[1] } else { ef[0] };
            v_in_next = level
                .face_edges(next_face)
                .iter()
                .position(|&x| x == next_edge)
                .unwrap_or(0);
        }
    }
    ring.len()
}

/// `getFacePoints`
fn face_points(level: &Level, f: Index, fvc: Option<usize>) -> &[Index] {
    match fvc {
        None => level.face_vertices(f),
        Some(c) => level.face_fvar_values(f, c),
    }
}

/// `getNextFaceInVertFaces`
fn next_face_in_vert_faces(
    level: &Level,
    this_in_vfaces: usize,
    vf: &[Index],
    vin: &[LocalIndex],
    manifold: bool,
) -> (Index, usize) {
    if manifold {
        let k = fast_mod_n(this_in_vfaces + 1, vf.len());
        (vf[k], vin[k] as usize)
    } else {
        let this_face = vf[this_in_vfaces];
        let v_in_this = vin[this_in_vfaces] as usize;
        let fe = level.face_edges(this_face);
        let next_edge = fe[fast_mod_n(v_in_this + fe.len() - 1, fe.len())];
        let ef = level.edge_faces(next_edge);
        let next = if ef[0] == this_face { ef[1] } else { ef[0] };
        let k = level
            .face_edges(next)
            .iter()
            .position(|&x| x == next_edge)
            .unwrap_or(0);
        (next, k)
    }
}

/// `getPrevFaceInVertFaces`
fn prev_face_in_vert_faces(
    level: &Level,
    this_in_vfaces: usize,
    vf: &[Index],
    vin: &[LocalIndex],
    manifold: bool,
) -> (Index, usize) {
    if manifold {
        let k = if this_in_vfaces > 0 {
            this_in_vfaces
        } else {
            vf.len()
        } - 1;
        (vf[k], vin[k] as usize)
    } else {
        let this_face = vf[this_in_vfaces];
        let v_in_this = vin[this_in_vfaces] as usize;
        let fe = level.face_edges(this_face);
        let prev_edge = fe[v_in_this];
        let ef = level.edge_faces(prev_edge);
        let prev = if ef[0] == this_face { ef[1] } else { ef[0] };
        let k = level
            .face_edges(prev)
            .iter()
            .position(|&x| x == prev_edge)
            .unwrap_or(0);
        (prev, fast_mod_n(k + 1, fe.len()))
    }
}

/// The patch builder over a refined hierarchy.
pub(crate) struct PatchBuilder<'a> {
    pub refiner: &'a TopologyRefiner,
}

impl PatchBuilder<'_> {
    /// `IsFaceAPatch`
    pub(crate) fn is_face_a_patch(&self, li: usize, f: Index) -> bool {
        let level = &self.refiner.levels[li];
        if self.refiner.has_holes && level.is_face_hole(f) {
            return false;
        }
        if li == 0 {
            return !level.face_composite_vtag(f).incid_irreg_face();
        }
        !level.face_composite_vtag(f).incomplete()
    }

    /// `IsFaceALeaf`
    pub(crate) fn is_face_a_leaf(&self, li: usize, f: Index) -> bool {
        if li < self.refiner.max_level() {
            let r = &self.refiner.refinements[li];
            if r.parent_face_tag
                .get(f as usize)
                .is_some_and(|t| t.selected)
            {
                return false;
            }
        }
        true
    }

    /// `IsPatchRegular` (`approxInfSharpWithSmooth = false`, legacy sharp corners).
    pub(crate) fn is_patch_regular(&self, li: usize, f: Index, fvc: Option<usize>) -> bool {
        let level = &self.refiner.levels[li];
        let mut tags = [VTag::default(); 4];
        level.face_vtags(f, fvc, &mut tags);
        let comp = VTag::bitwise_or(&tags);
        if !comp.inf_sharp() && !comp.inf_sharp_edges() {
            return !comp.xordinary();
        }
        let test_inf_sharp = true;
        let mut irreg = VTag::default();
        irreg.set_non_manifold(true);
        irreg.set_xordinary(true);
        irreg.set_inf_irregular(test_inf_sharp);
        let irreg_mask = irreg.0;
        if comp.0 & irreg_mask == 0 {
            return true;
        }
        let needs_extra = (comp.xordinary() && self.refiner.has_irreg_faces) as usize;
        let isolated = li > needs_extra;
        if isolated {
            let further = comp.non_manifold()
                || (comp.xordinary() && comp.boundary())
                || (test_inf_sharp && comp.inf_irregular() && comp.inf_sharp_edges());
            if !further {
                return !comp.inf_irregular();
            }
        }
        for i in 0..4 {
            let t = tags[i];
            if t.0 & irreg_mask == 0 {
                continue;
            }
            if t.non_manifold() {
                let n = identify_non_manifold_corner_span(
                    level,
                    f,
                    i,
                    singular_edge_mask(test_inf_sharp),
                    fvc,
                )
                .num_faces;
                if t.inf_sharp() {
                    if n != 1 {
                        return false;
                    }
                } else if n != 2 {
                    return false;
                }
                continue;
            }
            if t.xordinary() {
                if !t.inf_sharp_edges() {
                    return false;
                }
                if t.boundary() && !t.inf_sharp() {
                    let mut et = [ETag::default(); 4];
                    level.face_etags(f, fvc, &mut et);
                    let prev = if i > 0 { i - 1 } else { 3 };
                    if et[i].boundary() && et[prev].boundary() {
                        continue;
                    }
                }
            }
            if t.inf_irregular() {
                if !t.inf_sharp_edges() {
                    return false;
                }
                if t.inf_sharp_crease() && t.boundary() {
                    return false;
                }
                let n = identify_manifold_corner_span(level, f, i, singular_edge_mask(true), fvc)
                    .num_faces;
                if t.inf_sharp_crease() {
                    if n != 2 {
                        return false;
                    }
                } else if n != 1 {
                    return false;
                }
            }
        }
        true
    }

    /// `GetRegularPatchBoundaryMask`
    pub(crate) fn regular_patch_boundary_mask(
        &self,
        li: usize,
        f: Index,
        fvc: Option<usize>,
    ) -> i32 {
        let level = &self.refiner.levels[li];
        let mut tags = [VTag::default(); 4];
        level.face_vtags(f, fvc, &mut tags);
        let comp = VTag::bitwise_or(&tags);
        if !comp.inf_sharp_edges() {
            return 0;
        }
        let mut et = [ETag::default(); 4];
        level.face_etags(f, fvc, &mut et);
        let mut feat = ETag::default();
        feat.set_boundary(true);
        feat.set_inf_sharp(true);
        feat.set_non_manifold(true);
        let m = feat.0;
        (0..4).fold(0, |a, i| a | (((et[i].0 & m != 0) as i32) << i))
    }

    /// `GetIrregularPatchCornerSpans`
    pub(crate) fn irregular_patch_corner_spans(
        &self,
        li: usize,
        f: Index,
        fvc: Option<usize>,
    ) -> [VSpan; 4] {
        let level = &self.refiner.levels[li];
        let mut tags = [VTag::default(); 4];
        level.face_vtags(f, fvc, &mut tags);
        let mut fvar_tags = [super::tags::ValueTag::default(); 4];
        if let Some(c) = fvc {
            level
                .fvar_level(c)
                .face_value_tags(level, f, &mut fvar_tags);
        }
        let test_inf_sharp = true;
        let mask = singular_edge_mask(test_inf_sharp);
        let mut spans = [VSpan::default(); 4];
        for i in 0..4 {
            let t = tags[i];
            let non_manifold = t.non_manifold();
            let mismatch = fvc.is_some() && fvar_tags[i].mismatch();
            let test_edges = test_inf_sharp && t.inf_sharp_edges() && t.rule() != RULE_DART;
            if test_edges || mismatch || non_manifold {
                spans[i] = if non_manifold {
                    identify_non_manifold_corner_span(level, f, i, mask, fvc)
                } else {
                    identify_manifold_corner_span(level, f, i, mask, fvc)
                };
            } else {
                spans[i] = VSpan::default();
            }
            if t.corner() {
                spans[i].sharp = if mismatch {
                    spans[i].num_faces == 1 || non_manifold
                } else {
                    true
                };
            } else if non_manifold {
                spans[i].sharp = t.inf_sharp();
            } else if test_inf_sharp {
                spans[i].sharp = if test_edges {
                    !t.inf_sharp_crease()
                } else {
                    t.inf_sharp()
                };
            }
            let smooth_corner = !spans[i].sharp;
            if smooth_corner && t.xordinary() && t.boundary() && !t.inf_sharp() && !t.non_manifold()
            {
                let n = if spans[i].is_assigned() {
                    spans[i].num_faces as usize
                } else {
                    level.vertex_faces(level.face_vertices(f)[i]).len()
                };
                spans[i].sharp = n == 1;
            }
        }
        spans
    }

    /// `getQuadRegularPatchPoints` (`fillMissingBoundaryPoints`).
    pub(crate) fn regular_patch_points(
        &self,
        li: usize,
        f: Index,
        boundary_mask: i32,
        fvc: Option<usize>,
    ) -> [Index; 16] {
        const PER_CORNER: [[usize; 4]; 4] =
            [[5, 4, 0, 1], [6, 2, 3, 7], [10, 11, 15, 14], [9, 13, 12, 8]];
        let level = &self.refiner.levels[li];
        let interior = boundary_mask == 0;
        let e_mask = boundary_mask;
        let fv = level.face_vertices(f);
        let fp = face_points(level, f, fvc);
        let boundary_point = if interior { INDEX_INVALID } else { fp[0] };
        let mut out = [INDEX_INVALID; 16];
        for i in 0..4 {
            let v = fv[i];
            let cpi = &PER_CORNER[i];
            let vf = level.vertex_faces(v);
            let vin = level.vertex_face_local_indices(v);
            let f_in_vfaces = vf.iter().position(|&x| x == f).unwrap_or(vf.len());
            let i3 = fast_mod4(i as i32 + 3);
            let interior_corner = interior || ((e_mask & (1 << i)) | (e_mask & (1 << i3))) == 0;
            if interior_corner {
                let k = fast_mod4(f_in_vfaces as i32 + 2) as usize;
                let f_opp = vf[k];
                let v_in_opp = vin[k] as i32;
                let opp = face_points(level, f_opp, fvc);
                out[cpi[1]] = opp[fast_mod4(v_in_opp + 1) as usize];
                out[cpi[2]] = opp[fast_mod4(v_in_opp + 2) as usize];
                out[cpi[3]] = opp[fast_mod4(v_in_opp + 3) as usize];
            } else if (e_mask & (1 << i)) != 0 && (e_mask & (1 << i3)) != 0 {
                out[cpi[1]] = boundary_point;
                out[cpi[2]] = boundary_point;
                out[cpi[3]] = boundary_point;
            } else if (e_mask & (1 << i)) != 0 {
                let manifold = !level.vertex_tag(v).non_manifold();
                let (fnext, v_in_next) =
                    next_face_in_vert_faces(level, f_in_vfaces, vf, vin, manifold);
                let np = face_points(level, fnext, fvc);
                out[cpi[1]] = np[fast_mod4(v_in_next as i32 + 3) as usize];
                out[cpi[2]] = boundary_point;
                out[cpi[3]] = boundary_point;
            } else {
                let manifold = !level.vertex_tag(v).non_manifold();
                let (fprev, v_in_prev) =
                    prev_face_in_vert_faces(level, f_in_vfaces, vf, vin, manifold);
                let pp = face_points(level, fprev, fvc);
                out[cpi[1]] = boundary_point;
                out[cpi[2]] = boundary_point;
                out[cpi[3]] = pp[fast_mod4(v_in_prev as i32 + 1) as usize];
            }
            out[cpi[0]] = fp[i];
        }
        out
    }

    /// `assembleIrregularSourcePatch`
    fn assemble_irregular_source_patch(
        &self,
        li: usize,
        f: Index,
        spans: &[VSpan; 4],
    ) -> SourcePatch {
        let level = &self.refiner.levels[li];
        let fv = level.face_vertices(f);
        let mut sp = SourcePatch::default();
        for c in 0..fv.len() {
            let t = level.vertex_tag(fv[c]);
            let pc = &mut sp.corners[c];
            if spans[c].is_assigned() {
                pc.num_faces = spans[c].num_faces as i32;
                pc.patch_face = spans[c].corner_in_span as i32;
                pc.boundary = !spans[c].periodic;
            } else {
                let vf = level.vertex_faces(fv[c]);
                pc.num_faces = vf.len() as i32;
                pc.patch_face = vf.iter().position(|&x| x == f).unwrap_or(vf.len()) as i32;
                pc.boundary = t.boundary();
            }
            pc.sharp = spans[c].sharp;
            pc.dart = t.rule() == RULE_DART && t.inf_sharp_edges();
        }
        sp.finalize(fv.len());
        sp
    }

    /// `GetIrregularPatchSourcePoints` (`gatherIrregularSourcePoints`).
    pub(crate) fn irregular_patch_source_points(
        &self,
        li: usize,
        f: Index,
        spans: &[VSpan; 4],
        fvc: Option<usize>,
    ) -> Vec<Index> {
        let sp = self.assemble_irregular_source_patch(li, f, spans);
        let level = &self.refiner.levels[li];
        let fv = level.face_vertices(f);
        let mut out = vec![0 as Index; sp.num_source_points as usize];
        let mut ring = Vec::new();
        let mut patch_ring = Vec::new();
        for c in 0..sp.num_corners {
            let v = fv[c];
            if spans[c].is_assigned() {
                gather_regular_partial_ring(level, v, &spans[c], &mut ring, fvc);
            } else {
                level.gather_quad_regular_ring_around_vertex(v, &mut ring, fvc);
            }
            sp.corner_ring_points(c, &mut patch_ring);
            for i in 0..patch_ring.len() {
                out[patch_ring[i] as usize] = ring[i];
            }
        }
        out
    }

    /// `GetIrregularPatchConversionMatrix<float>` (Gregory basis).
    pub(crate) fn irregular_patch_conversion_matrix(
        &self,
        li: usize,
        f: Index,
        spans: &[VSpan; 4],
    ) -> SparseMatrix {
        let sp = self.assemble_irregular_source_patch(li, f, spans);
        gregory::convert(&sp)
    }

    /// `ComputePatchParam`
    pub(crate) fn compute_patch_param(
        &self,
        li: usize,
        f: Index,
        ptex: &[i32],
        is_regular: bool,
        boundary_mask: i32,
        compute_transition: bool,
    ) -> PatchParam {
        let depth = li;
        let mut child_index_in_parent = 0;
        let (mut u, mut v) = (0i32, 0i32);
        let mut ofs = 1i32;
        let levels = &self.refiner.levels;
        let mut irreg_base = levels[depth].num_face_vertices(f) != 4;
        let mut child_face = f;
        for i in (1..=depth).rev() {
            let r = &self.refiner.refinements[i - 1];
            let parent_level = &levels[i - 1];
            let parent_face = r.child_face_parent_face(child_face);
            irreg_base = parent_level.num_face_vertices(parent_face) != 4;
            if !irreg_base {
                child_index_in_parent = r.child_face_in_parent_face(child_face);
                match child_index_in_parent {
                    1 => u += ofs,
                    2 => {
                        u += ofs;
                        v += ofs;
                    }
                    3 => v += ofs,
                    _ => {}
                }
                ofs <<= 1;
            } else {
                let children = r.face_child_faces(parent_face);
                if let Some(j) = children.iter().position(|&c| c == child_face) {
                    child_index_in_parent = j as i32;
                }
            }
            child_face = parent_face;
        }
        let mut ptex_index = ptex[child_face as usize];
        if irreg_base {
            ptex_index += child_index_in_parent;
        }
        let mut transition = 0;
        if compute_transition && li < self.refiner.max_level() {
            transition = self.refiner.refinements[li]
                .parent_face_tag
                .get(f as usize)
                .map_or(0, |t| t.transitional as i32);
        }
        PatchParam {
            face_id: ptex_index,
            u,
            v,
            depth: depth as i32,
            non_quad_root: irreg_base,
            boundary: boundary_mask,
            transition,
            regular: is_regular,
        }
    }
}
