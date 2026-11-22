// SPDX-License-Identifier: GPL-3.0-or-later
//! `Sdc`: the options Blender passes (`getSDCOptions`: uniform creasing, the vertex boundary
//! and face-varying linear interpolation of the modifier), the sharpness rules of
//! `Sdc::Crease` and the Catmull-Clark masks of `Sdc::Scheme<SCHEME_CATMARK>`, all in `f32`
//! (OpenSubdiv's `StencilTableReal<float>` instantiates them with `float` weights).
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/sdc/{options.h,crease.h,crease.cpp,scheme.h,
//! catmarkScheme.h}` (Pixar, TOST / Apache-2.0 modified). Where clang contracts `a * b + c`
//! inside one expression (Blender's arm64 build of OpenSubdiv, `-ffp-contract=on`), the port
//! uses `f32::mul_add`.

use super::tags::{RULE_CORNER, RULE_CREASE, RULE_DART, RULE_SMOOTH, RULE_UNKNOWN};

/// `Sdc::Crease::SHARPNESS_SMOOTH`
pub(crate) const SHARPNESS_SMOOTH: f32 = 0.0;
/// `Sdc::Crease::SHARPNESS_INFINITE`
pub(crate) const SHARPNESS_INFINITE: f32 = 10.0;

/// `Sdc::Options::VtxBoundaryInterpolation`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum VtxBoundary {
    /// `VTX_BOUNDARY_NONE`
    None,
    /// `VTX_BOUNDARY_EDGE_ONLY`
    #[default]
    EdgeOnly,
    /// `VTX_BOUNDARY_EDGE_AND_CORNER`
    EdgeAndCorner,
}

/// `Sdc::Options::FVarLinearInterpolation`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum FVarLinear {
    /// `FVAR_LINEAR_NONE`
    None,
    /// `FVAR_LINEAR_CORNERS_ONLY`
    CornersOnly,
    /// `FVAR_LINEAR_CORNERS_PLUS1`
    CornersPlus1,
    /// `FVAR_LINEAR_CORNERS_PLUS2`
    CornersPlus2,
    /// `FVAR_LINEAR_BOUNDARIES`
    #[default]
    Boundaries,
    /// `FVAR_LINEAR_ALL`
    All,
}

/// `Sdc::Options` with `CREASE_UNIFORM` and `TRI_SUB_CATMARK` (Blender's choices).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SdcOptions {
    pub vtx_boundary: VtxBoundary,
    pub fvar_linear: FVarLinear,
}

#[inline]
pub(crate) fn is_smooth(s: f32) -> bool {
    s <= SHARPNESS_SMOOTH
}
#[inline]
pub(crate) fn is_sharp(s: f32) -> bool {
    s > SHARPNESS_SMOOTH
}
#[inline]
pub(crate) fn is_infinite(s: f32) -> bool {
    s >= SHARPNESS_INFINITE
}
#[inline]
pub(crate) fn is_semi_sharp(s: f32) -> bool {
    SHARPNESS_SMOOTH < s && s < SHARPNESS_INFINITE
}

/// `Crease::decrementSharpness` (`SubdivideUniformSharpness`, `SubdivideVertexSharpness`).
#[inline]
pub(crate) fn decrement_sharpness(s: f32) -> f32 {
    if is_smooth(s) {
        SHARPNESS_SMOOTH
    } else if is_infinite(s) {
        SHARPNESS_INFINITE
    } else if s > 1.0 {
        s - 1.0
    } else {
        SHARPNESS_SMOOTH
    }
}

/// `Crease::DetermineVertexVertexRule(vertexSharpness, sharpEdgeCount)`
pub(crate) fn rule_from_count(vertex_sharpness: f32, sharp_edge_count: i32) -> u8 {
    if is_sharp(vertex_sharpness) {
        return RULE_CORNER;
    }
    if sharp_edge_count > 2 {
        RULE_CORNER
    } else {
        1 << sharp_edge_count
    }
}

/// `Crease::DetermineVertexVertexRule(vertexSharpness, count, edgeSharpness)`
pub(crate) fn rule_from_sharpness(vertex_sharpness: f32, edge_sharpness: &[f32]) -> u8 {
    if is_sharp(vertex_sharpness) {
        return RULE_CORNER;
    }
    let n = edge_sharpness.iter().filter(|&&s| is_sharp(s)).count() as i32;
    if n > 2 { RULE_CORNER } else { 1 << n }
}

/// `Crease::ComputeFractionalWeightAtVertex` (uniform creasing).
pub(crate) fn fractional_weight_at_vertex(
    parent_vertex_sharpness: f32,
    child_vertex_sharpness: f32,
    parent_sharpness: &[f32],
) -> f32 {
    let mut count = 0i32;
    let mut sum = 0.0f32;
    if is_sharp(parent_vertex_sharpness) && is_smooth(child_vertex_sharpness) {
        count = 1;
        sum = parent_vertex_sharpness;
    }
    for &p in parent_sharpness {
        if is_sharp(p) && p <= 1.0 {
            sum += p;
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    let w = sum / count as f32;
    if w > 1.0 { 1.0 } else { w }
}

/// `Crease::GetSharpEdgePairOfCrease`
pub(crate) fn sharp_edge_pair(edge_sharpness: &[f32]) -> [usize; 2] {
    let mut a = 0;
    while is_smooth(edge_sharpness[a]) {
        a += 1;
    }
    let mut b = edge_sharpness.len() - 1;
    while is_smooth(edge_sharpness[b]) {
        b -= 1;
    }
    [a, b]
}

/// A mask (`Sdc` `MASK` concept) over the vertex, edge and face neighbours of a component.
#[derive(Clone, Debug, Default)]
pub(crate) struct Mask {
    pub v: Vec<f32>,
    pub e: Vec<f32>,
    pub f: Vec<f32>,
    pub nv: usize,
    pub ne: usize,
    pub nf: usize,
    pub face_centers: bool,
}

impl Mask {
    fn set(&mut self, nv: usize, ne: usize, nf: usize, centers: bool) {
        self.nv = nv;
        self.ne = ne;
        self.nf = nf;
        self.face_centers = centers;
        if self.v.len() < nv {
            self.v.resize(nv, 0.0);
        }
        if self.e.len() < ne {
            self.e.resize(ne, 0.0);
        }
        if self.f.len() < nf {
            self.f.resize(nf, 0.0);
        }
    }

    /// `LocalMask::CombineVertexVertexMasks(thisCoeff, dstCoeff, dst)` with `self` the child
    /// mask.
    fn combine_into(&self, this_coeff: f32, dst_coeff: f32, dst: &mut Mask) {
        dst.v[0] = dst_coeff.mul_add(dst.v[0], this_coeff * self.v[0]);
        let ne = self.ne;
        if ne > 0 {
            if dst.ne == 0 {
                dst.set(dst.nv, ne, dst.nf, dst.face_centers);
                for i in 0..ne {
                    dst.e[i] = this_coeff * self.e[i];
                }
            } else {
                for i in 0..ne {
                    dst.e[i] = dst_coeff.mul_add(dst.e[i], this_coeff * self.e[i]);
                }
            }
        }
        let nf = self.nf;
        if nf > 0 {
            if dst.nf == 0 {
                let centers = self.face_centers;
                dst.set(dst.nv, dst.ne, nf, centers);
                for i in 0..nf {
                    dst.f[i] = this_coeff * self.f[i];
                }
            } else {
                for i in 0..nf {
                    dst.f[i] = dst_coeff.mul_add(dst.f[i], this_coeff * self.f[i]);
                }
            }
        }
    }
}

/// `Scheme::ComputeFaceVertexMask`
pub(crate) fn face_vertex_mask(vert_count: usize, m: &mut Mask) {
    m.set(vert_count, 0, 0, false);
    let w = 1.0f32 / vert_count as f32;
    for i in 0..vert_count {
        m.v[i] = w;
    }
}

/// `Scheme<CATMARK>::assignSmoothMaskForEdge` (no triangle option).
fn smooth_mask_for_edge(face_count: usize, m: &mut Mask) {
    m.set(2, 0, face_count, true);
    m.v[0] = 0.25;
    m.v[1] = 0.25;
    if face_count == 2 {
        m.f[0] = 0.25;
        m.f[1] = 0.25;
    } else {
        let w = 0.5f32 / face_count as f32;
        for i in 0..face_count {
            m.f[i] = w;
        }
    }
}

/// `Scheme::assignCreaseMaskForEdge`
fn crease_mask_for_edge(m: &mut Mask) {
    m.set(2, 0, 0, false);
    m.v[0] = 0.5;
    m.v[1] = 0.5;
}

/// `Scheme::ComputeEdgeVertexMask` (uniform creasing).
pub(crate) fn edge_vertex_mask(
    face_count: usize,
    edge_sharpness: f32,
    parent_rule: u8,
    child_rule: u8,
    m: &mut Mask,
) {
    if parent_rule == RULE_SMOOTH || (parent_rule == RULE_UNKNOWN && edge_sharpness <= 0.0) {
        smooth_mask_for_edge(face_count, m);
        return;
    }
    if child_rule == RULE_CREASE {
        crease_mask_for_edge(m);
        return;
    }
    if child_rule == RULE_UNKNOWN {
        let child_is_crease = parent_rule == RULE_CREASE || edge_sharpness >= 1.0;
        if child_is_crease {
            crease_mask_for_edge(m);
            return;
        }
    }
    smooth_mask_for_edge(face_count, m);
    let p = edge_sharpness;
    let c = 1.0f32 - p;
    m.v[0] = p.mul_add(0.5, c * m.v[0]);
    m.v[1] = p.mul_add(0.5, c * m.v[1]);
    for i in 0..m.nf {
        m.f[i] *= c;
    }
}

/// `Scheme::assignCornerMaskForVertex`
fn corner_mask_for_vertex(m: &mut Mask) {
    m.set(1, 0, 0, false);
    m.v[0] = 1.0;
}

/// `Scheme<CATMARK>::assignCreaseMaskForVertex`
fn crease_mask_for_vertex(valence: usize, ends: [usize; 2], m: &mut Mask) {
    m.set(1, valence, 0, false);
    m.v[0] = 0.75;
    for i in 0..valence {
        m.e[i] = 0.0;
    }
    m.e[ends[0]] = 0.125;
    m.e[ends[1]] = 0.125;
}

/// `Scheme<CATMARK>::assignSmoothMaskForVertex`
fn smooth_mask_for_vertex(valence: usize, m: &mut Mask) {
    m.set(1, valence, valence, true);
    let v = (valence as i32 - 2) as f32 / valence as f32;
    let f = 1.0f32 / (valence * valence) as f32;
    m.v[0] = v;
    for i in 0..valence {
        m.e[i] = f;
        m.f[i] = f;
    }
}

/// The parent-vertex neighbourhood `Scheme::ComputeVertexVertexMask` reads
/// (`Vtr::internal::VertexInterface`).
pub(crate) struct VertexHood<'a> {
    pub num_edges: usize,
    pub num_faces: usize,
    pub sharpness: f32,
    pub edge_sharpness: &'a [f32],
    pub child_sharpness: f32,
}

/// `Scheme::ComputeVertexVertexMask` (uniform creasing).
pub(crate) fn vertex_vertex_mask(hood: &VertexHood<'_>, m: &mut Mask, p_rule: u8, c_rule: u8) {
    let mut c_rule = c_rule;
    let mut p_rule = p_rule;
    if p_rule == RULE_SMOOTH || p_rule == RULE_DART {
        smooth_mask_for_vertex(hood.num_faces, m);
        return;
    }
    if c_rule == RULE_UNKNOWN && p_rule != RULE_UNKNOWN {
        c_rule = p_rule;
    }
    let valence = hood.num_edges;
    let require_parent = p_rule == RULE_UNKNOWN || p_rule == RULE_CREASE || p_rule != c_rule;
    let mut p_vertex_sharpness = 0.0f32;
    let p_edge_sharpness = hood.edge_sharpness;
    if require_parent {
        p_vertex_sharpness = hood.sharpness;
        if p_rule == RULE_UNKNOWN {
            p_rule = rule_from_sharpness(p_vertex_sharpness, p_edge_sharpness);
        }
    }
    if p_rule == RULE_SMOOTH || p_rule == RULE_DART {
        smooth_mask_for_vertex(hood.num_faces, m);
        return;
    } else if p_rule == RULE_CREASE {
        crease_mask_for_vertex(valence, sharp_edge_pair(p_edge_sharpness), m);
    } else {
        corner_mask_for_vertex(m);
    }
    if c_rule == p_rule {
        return;
    }
    let c_edge_sharpness: Vec<f32> = p_edge_sharpness
        .iter()
        .map(|&s| decrement_sharpness(s))
        .collect();
    let c_vertex_sharpness = hood.child_sharpness;
    if c_rule == RULE_UNKNOWN {
        c_rule = rule_from_sharpness(c_vertex_sharpness, &c_edge_sharpness);
        if c_rule == p_rule {
            return;
        }
    }
    let mut c_mask = Mask::default();
    if c_rule == RULE_SMOOTH || c_rule == RULE_DART {
        smooth_mask_for_vertex(hood.num_faces, &mut c_mask);
    } else if c_rule == RULE_CREASE {
        crease_mask_for_vertex(valence, sharp_edge_pair(&c_edge_sharpness), &mut c_mask);
    } else {
        corner_mask_for_vertex(&mut c_mask);
    }
    let p_weight =
        fractional_weight_at_vertex(p_vertex_sharpness, c_vertex_sharpness, p_edge_sharpness);
    let c_weight = 1.0f32 - p_weight;
    c_mask.combine_into(c_weight, p_weight, m);
}
