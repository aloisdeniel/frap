// SPDX-License-Identifier: GPL-3.0-or-later
//! Subdivision core rules (`Sdc`): crease sharpness, vertex rules and the Catmull-Clark masks
//! of OpenSubdiv's `Sdc::Scheme<SCHEME_CATMARK>` with `CREASE_UNIFORM`, the creasing method
//! Blender selects (`intern/opensubdiv/internal/topology/topology_refiner_factory.cc`,
//! `getSDCOptions`).
//!
//! Ported from OpenSubdiv 3.x `opensubdiv/sdc/{crease.h,crease.cpp,scheme.h,catmarkScheme.h}`
//! (Pixar, Apache-2.0 modified, "TOST"), the library Blender links for subdivision surfaces.

/// `Sdc::Crease::SHARPNESS_SMOOTH`
pub(crate) const SHARPNESS_SMOOTH: f32 = 0.0;
/// `Sdc::Crease::SHARPNESS_INFINITE`
pub(crate) const SHARPNESS_INFINITE: f32 = 10.0;

/// `Sdc::Crease::IsSmooth`
#[inline]
pub(crate) fn is_smooth(s: f32) -> bool {
    s <= SHARPNESS_SMOOTH
}

/// `Sdc::Crease::IsSharp`
#[inline]
pub(crate) fn is_sharp(s: f32) -> bool {
    s > SHARPNESS_SMOOTH
}

/// `Sdc::Crease::IsInfinite`
#[inline]
pub(crate) fn is_infinite(s: f32) -> bool {
    s >= SHARPNESS_INFINITE
}

/// `Sdc::Crease::IsSemiSharp`
#[inline]
pub(crate) fn is_semi_sharp(s: f32) -> bool {
    SHARPNESS_SMOOTH < s && s < SHARPNESS_INFINITE
}

/// `Sdc::Crease::decrementSharpness` (uniform creasing: vertex and edge sharpness alike).
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

/// `Sdc::Crease::Rule`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum Rule {
    /// `RULE_UNKNOWN`
    #[default]
    Unknown,
    /// `RULE_SMOOTH`
    Smooth,
    /// `RULE_DART`
    Dart,
    /// `RULE_CREASE`
    Crease,
    /// `RULE_CORNER`
    Corner,
}

impl Rule {
    /// `Crease::DetermineVertexVertexRule(vertexSharpness, sharpEdgeCount)`.
    pub(crate) fn determine(vertex_sharpness: f32, sharp_edge_count: usize) -> Rule {
        if is_sharp(vertex_sharpness) {
            return Rule::Corner;
        }
        match sharp_edge_count {
            0 => Rule::Smooth,
            1 => Rule::Dart,
            2 => Rule::Crease,
            _ => Rule::Corner,
        }
    }
}

/// `Crease::ComputeFractionalWeightAtVertex` for uniform creasing.
pub(crate) fn fractional_weight_at_vertex(
    parent_vertex_sharpness: f32,
    child_vertex_sharpness: f32,
    parent_edge_sharpness: &[f32],
) -> f32 {
    let mut count = 0u32;
    let mut sum = 0.0f32;
    if is_sharp(parent_vertex_sharpness) && is_smooth(child_vertex_sharpness) {
        count = 1;
        sum = parent_vertex_sharpness;
    }
    for &s in parent_edge_sharpness {
        if is_sharp(s) && s <= 1.0 {
            sum += s;
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    let w = sum / count as f32;
    if w > 1.0 { 1.0 } else { w }
}

/// A subdivision or limit mask over a vertex neighbourhood: one vertex weight, one weight per
/// incident edge (applied to the vertex at the other end) and one per incident face (applied to
/// the face's child vertex, or to the vertex opposite in the face for limit masks:
/// `SetFaceWeightsForFaceCenters`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Mask {
    pub vertex: f64,
    pub edges: Vec<f64>,
    pub faces: Vec<f64>,
}

/// `Scheme<CATMARK>::assignSmoothMaskForVertex` (face weights for face centres).
pub(crate) fn smooth_vertex_mask(valence: usize, m: &mut Mask) {
    let n = valence as f64;
    m.vertex = (n - 2.0) / n;
    let w = 1.0 / (n * n);
    m.edges.clear();
    m.edges.resize(valence, w);
    m.faces.clear();
    m.faces.resize(valence, w);
}

/// `Scheme<CATMARK>::assignCreaseMaskForVertex`: 3/4 the vertex, 1/8 each crease end.
pub(crate) fn crease_vertex_mask(n_edges: usize, ends: [usize; 2], m: &mut Mask) {
    m.vertex = 0.75;
    m.edges.clear();
    m.edges.resize(n_edges, 0.0);
    m.edges[ends[0]] = 0.125;
    m.edges[ends[1]] = 0.125;
    m.faces.clear();
}

/// `Scheme::assignCornerMaskForVertex`
pub(crate) fn corner_vertex_mask(m: &mut Mask) {
    m.vertex = 1.0;
    m.edges.clear();
    m.faces.clear();
}

/// `Crease::GetSharpEdgePairOfCrease`: the first and last sharp edges around the vertex.
pub(crate) fn sharp_edge_pair(edge_sharpness: &[f32]) -> [usize; 2] {
    let mut a = 0;
    while a < edge_sharpness.len() && is_smooth(edge_sharpness[a]) {
        a += 1;
    }
    let mut b = edge_sharpness.len().saturating_sub(1);
    while b > 0 && is_smooth(edge_sharpness[b]) {
        b -= 1;
    }
    [a.min(edge_sharpness.len().saturating_sub(1)), b]
}

/// `Scheme::ComputeVertexVertexMask` with both rules known (the parent rule from the parent
/// vertex's tag, the child rule from the child's), blending a crease/corner parent into a
/// smoother child by the fractional weight.
#[allow(clippy::too_many_arguments)] // OpenSubdiv's vertex neighbourhood, flattened
pub(crate) fn vertex_vertex_mask(
    valence_faces: usize,
    p_rule: Rule,
    c_rule: Rule,
    p_vertex_sharpness: f32,
    c_vertex_sharpness: f32,
    p_edge_sharpness: &[f32],
    c_edge_sharpness: &[f32],
    m: &mut Mask,
) {
    if matches!(p_rule, Rule::Smooth | Rule::Dart) {
        smooth_vertex_mask(valence_faces, m);
        return;
    }
    let n_edges = p_edge_sharpness.len();
    if p_rule == Rule::Crease {
        crease_vertex_mask(n_edges, sharp_edge_pair(p_edge_sharpness), m);
    } else {
        corner_vertex_mask(m);
    }
    if c_rule == p_rule {
        return;
    }
    let mut c = Mask::default();
    match c_rule {
        Rule::Smooth | Rule::Dart => smooth_vertex_mask(valence_faces, &mut c),
        Rule::Crease => crease_vertex_mask(n_edges, sharp_edge_pair(c_edge_sharpness), &mut c),
        _ => corner_vertex_mask(&mut c),
    }
    let p_weight =
        fractional_weight_at_vertex(p_vertex_sharpness, c_vertex_sharpness, p_edge_sharpness)
            as f64;
    let c_weight = 1.0 - p_weight;
    // `LocalMask::CombineVertexVertexMasks(cWeight, pWeight, mask)`.
    m.vertex = p_weight * m.vertex + c_weight * c.vertex;
    if !c.edges.is_empty() {
        if m.edges.is_empty() {
            m.edges = c.edges.iter().map(|w| c_weight * w).collect();
        } else {
            for (d, s) in m.edges.iter_mut().zip(&c.edges) {
                *d = p_weight * *d + c_weight * s;
            }
        }
    }
    if !c.faces.is_empty() {
        if m.faces.is_empty() {
            m.faces = c.faces.iter().map(|w| c_weight * w).collect();
        } else {
            for (d, s) in m.faces.iter_mut().zip(&c.faces) {
                *d = p_weight * *d + c_weight * s;
            }
        }
    }
}

/// `Scheme::ComputeEdgeVertexMask` for Catmark: returns (vertex weight each, face-centre
/// weight each); `face_count` incident faces.
pub(crate) fn edge_vertex_mask(
    sharpness: f32,
    p_rule_crease: bool,
    c_rule: Rule,
    face_count: usize,
) -> (f64, f64) {
    let smooth = || {
        if face_count == 2 {
            (0.25, 0.25)
        } else {
            (0.25, 0.5 / face_count as f64)
        }
    };
    if !p_rule_crease {
        return smooth();
    }
    if c_rule == Rule::Crease || sharpness >= 1.0 {
        return (0.5, 0.0);
    }
    // Uniform creasing: sharpness < 1 always decays, so this is the crease-to-smooth blend.
    let (v, f) = smooth();
    let p = sharpness as f64;
    let c = 1.0 - p;
    (p * 0.5 + c * v, f * c)
}

/// `Scheme<CATMARK>::ComputeVertexLimitMask` (position only). Face weights apply to the
/// vertex opposite in each (quad) face.
///
/// Blender evaluates limit positions through patches, whose corner points come from
/// `CatmarkLimits::ComputeInteriorPointWeights` (Gregory end caps): the same weights as Sdc's
/// for valence 3 and up, but `Sdc` maps an interior valence-2 vertex to its corner mask while
/// the patch formula applies `n / (n + 5)` there too; this follows the patches.
pub(crate) fn vertex_limit_mask(
    valence_faces: usize,
    rule: Rule,
    edge_sharpness: &[f32],
    m: &mut Mask,
) {
    match rule {
        Rule::Smooth | Rule::Dart => {
            m.edges.clear();
            m.faces.clear();
            if valence_faces == 4 {
                m.vertex = 4.0 / 9.0;
                m.edges.resize(4, 1.0 / 9.0);
                m.faces.resize(4, 1.0 / 36.0);
            } else {
                let n = valence_faces as f64;
                let f = 1.0 / (n * (n + 5.0));
                let e = 4.0 * f;
                m.vertex = 1.0 - n * (e + f);
                m.edges.resize(valence_faces, e);
                m.faces.resize(valence_faces, f);
            }
        }
        Rule::Crease => {
            let ends = sharp_edge_pair(edge_sharpness);
            m.vertex = 2.0 / 3.0;
            m.edges.clear();
            m.edges.resize(edge_sharpness.len(), 0.0);
            m.edges[ends[0]] = 1.0 / 6.0;
            m.edges[ends[1]] = 1.0 / 6.0;
            m.faces.clear();
        }
        _ => corner_vertex_mask(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrement_and_rules() {
        assert_eq!(decrement_sharpness(2.5), 1.5);
        assert_eq!(decrement_sharpness(0.5), 0.0);
        assert_eq!(decrement_sharpness(10.0), 10.0);
        assert_eq!(Rule::determine(0.0, 0), Rule::Smooth);
        assert_eq!(Rule::determine(0.0, 1), Rule::Dart);
        assert_eq!(Rule::determine(0.0, 2), Rule::Crease);
        assert_eq!(Rule::determine(0.0, 3), Rule::Corner);
        assert_eq!(Rule::determine(0.1, 0), Rule::Corner);
    }

    #[test]
    fn limit_masks_sum_to_one() {
        for n in [2usize, 3, 4, 5, 7] {
            let mut m = Mask::default();
            vertex_limit_mask(n, Rule::Smooth, &vec![0.0; n], &mut m);
            let s: f64 = m.vertex + m.edges.iter().sum::<f64>() + m.faces.iter().sum::<f64>();
            assert!((s - 1.0).abs() < 1e-12, "{n}: {s}");
        }
    }

    #[test]
    fn crease_to_smooth_blend() {
        let mut m = Mask::default();
        // Two incident edges at sharpness 0.5 of four: half crease, half smooth.
        vertex_vertex_mask(
            4,
            Rule::Crease,
            Rule::Smooth,
            0.0,
            0.0,
            &[0.5, 0.0, 0.5, 0.0],
            &[0.0; 4],
            &mut m,
        );
        assert!((m.vertex - (0.5 * 0.75 + 0.5 * 0.5)).abs() < 1e-12);
        assert!((m.edges[0] - (0.5 * 0.125 + 0.5 / 16.0)).abs() < 1e-12);
        assert!((m.faces[1] - 0.5 / 16.0).abs() < 1e-12);
    }
}
