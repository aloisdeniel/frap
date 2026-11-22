// SPDX-License-Identifier: GPL-3.0-or-later
//! `Far::TopologyRefiner`: the base level as Blender's `TopologyRefinerFactory
//! <TopologyRefinerData>` builds it (partial topology, Blender's edge and vertex sharpness,
//! the UV maps as face-varying channels), and uniform or feature-adaptive refinement
//! (`RefineUniform`, `RefineAdaptive` with `considerFVarChannels` and `useInfSharpPatch`).
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/far/{topologyRefiner,topologyRefinerFactory}.
//! {h,cpp}` (Pixar, TOST / Apache-2.0 modified) and Blender 5.2.4
//! `intern/opensubdiv/internal/topology/topology_refiner_factory.cc` (GPL-2.0-or-later).

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::fvar_level::FVarLevel;
use super::level::Level;
use super::refinement::Refinement;
use super::sdc::{self, SdcOptions, VtxBoundary};
use super::tags::{RULE_CORNER, RULE_CREASE, RULE_DART, RULE_SMOOTH, VTag};
use super::{INDEX_INVALID, Index, VALENCE_LIMIT};

/// The base mesh as Blender's converter hands it to OpenSubdiv.
#[derive(Clone, Debug, Default)]
pub(crate) struct BaseMesh {
    pub num_vertices: i32,
    /// Vertices per face.
    pub face_sizes: Vec<i32>,
    /// Face-vertices (base-level indices).
    pub face_verts: Vec<Index>,
    /// `(v0, v1, sharpness)` of every manifold edge with a sharpness of at least `1e-6`, in
    /// Blender's edge order (`assignComponentTags`).
    pub edge_sharpness: Vec<([Index; 2], f32)>,
    /// `isInfiniteSharpVertex` per base vertex.
    pub inf_sharp_verts: Vec<bool>,
    /// `getVertexSharpness` per base vertex.
    pub vert_sharpness: Vec<f32>,
    /// Face-varying channels: per face corner the value index, and the number of values.
    pub fvar: Vec<(Vec<Index>, i32)>,
}

/// The refined hierarchy.
#[derive(Clone, Debug, Default)]
pub(crate) struct TopologyRefiner {
    pub(crate) is_uniform: bool,
    pub(crate) has_holes: bool,
    pub(crate) has_irreg_faces: bool,
    pub(crate) levels: Vec<Level>,
    pub(crate) refinements: Vec<Refinement>,
}

/// Why the refiner could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RefinerError(pub String);

/// `internal::FeatureMask` for Blender's adaptive options.
#[derive(Copy, Clone, Debug)]
struct FeatureMask {
    select_xordinary_interior: bool,
    select_xordinary_boundary: bool,
    select_semi_sharp_single: bool,
    select_semi_sharp_non_single: bool,
    select_inf_sharp_regular_crease: bool,
    select_inf_sharp_regular_corner: bool,
    select_inf_sharp_irregular_dart: bool,
    select_inf_sharp_irregular_crease: bool,
    select_inf_sharp_irregular_corner: bool,
    select_unisolated_interior_edge: bool,
    select_non_manifold: bool,
    select_fvar_features: bool,
}

impl FeatureMask {
    /// `FeatureMask::InitializeFeatures` with `useSingleCreasePatch = false`.
    fn new(use_inf_sharp_patch: bool, consider_fvar: bool) -> FeatureMask {
        FeatureMask {
            select_xordinary_interior: true,
            select_xordinary_boundary: true,
            select_semi_sharp_single: true,
            select_semi_sharp_non_single: true,
            select_inf_sharp_regular_crease: !use_inf_sharp_patch,
            select_inf_sharp_regular_corner: !use_inf_sharp_patch,
            select_inf_sharp_irregular_dart: true,
            select_inf_sharp_irregular_crease: true,
            select_inf_sharp_irregular_corner: true,
            select_unisolated_interior_edge: false,
            select_non_manifold: true,
            select_fvar_features: consider_fvar,
        }
    }
}

impl TopologyRefiner {
    /// `TopologyRefinerFactory<TopologyRefinerData>::Create`
    pub(crate) fn create(
        mesh: &BaseMesh,
        options: SdcOptions,
    ) -> Result<TopologyRefiner, RefinerError> {
        let mut r = TopologyRefiner {
            is_uniform: true,
            ..TopologyRefiner::default()
        };
        let mut base = Level::default();
        // resizeComponentTopology
        base.resize_vertices(mesh.num_vertices);
        base.resize_faces(mesh.face_sizes.len() as i32);
        for (f, &n) in mesh.face_sizes.iter().enumerate() {
            base.resize_face_vertices_of(f as Index, n);
            r.has_irreg_faces |= n != 4;
        }
        // prepareComponentTopologySizing
        if base.vert_count == 0 {
            return Err(RefinerError("mesh contains no vertices".into()));
        }
        if base.face_count == 0 {
            return Err(RefinerError(
                "meshes without faces not yet supported".into(),
            ));
        }
        if base.max_valence > VALENCE_LIMIT {
            return Err(RefinerError(format!(
                "face with {} vertices > {} max",
                base.max_valence, VALENCE_LIMIT
            )));
        }
        // assignComponentTopology
        base.face_vert_indices = mesh.face_verts.clone();
        // prepareComponentTopologyAssignment
        if !base.complete_topology_from_face_vertices() {
            return Err(RefinerError(format!(
                "vertex with valence {} > {} max",
                base.max_valence, VALENCE_LIMIT
            )));
        }
        // assignComponentTags (Blender)
        for &(ev, s) in &mesh.edge_sharpness {
            if s < 1e-6 {
                continue;
            }
            let e = base.find_edge(ev[0], ev[1]);
            if e == INDEX_INVALID {
                return Err(RefinerError("failed to find reconstructed edge".into()));
            }
            base.edge_sharpness[e as usize] = s;
        }
        for v in 0..base.vert_count {
            if mesh.inf_sharp_verts[v as usize] {
                base.vert_sharpness[v as usize] = sdc::SHARPNESS_INFINITE;
                continue;
            }
            let mut s = mesh.vert_sharpness[v as usize];
            let ve = base.vertex_edges(v);
            if ve.len() == 2 {
                let s0 = base.edge_sharpness[ve[0] as usize];
                let s1 = base.edge_sharpness[ve[1] as usize];
                s += s0.min(s1);
                s = s.min(10.0);
            }
            base.vert_sharpness[v as usize] = s;
        }
        prepare_component_tags_and_sharpness(&mut base, options, r.has_irreg_faces);
        // assignFaceVaryingTopology + prepareFaceVaryingChannels
        for (values, n) in &mesh.fvar {
            let mut fv = FVarLevel {
                options,
                value_count: *n,
                ..FVarLevel::default()
            };
            fv.resize_components(&base);
            fv.face_vert_values.copy_from_slice(values);
            if *n == 0 {
                return Err(RefinerError("face-varying channel has no values".into()));
            }
            fv.complete_topology_from_face_values(&base, 2);
            base.fvar.push(fv);
        }
        r.levels.push(base);
        Ok(r)
    }

    pub(crate) fn max_level(&self) -> usize {
        self.refinements.len()
    }

    /// `RefineUniform(UniformOptions(level))` (`fullTopologyInLastLevel = false`).
    pub(crate) fn refine_uniform(&mut self, level: u32) {
        self.is_uniform = true;
        for i in 1..=level {
            let minimal = i == level;
            let mut child = Level::default();
            let mut r = Refinement::default();
            r.refine(&self.levels[i as usize - 1], &mut child, false, minimal);
            self.levels.push(child);
            self.refinements.push(r);
        }
    }

    /// `RefineAdaptive(AdaptiveOptions(level))` with Blender's `considerFVarChannels` and
    /// `useInfSharpPatch = true`.
    pub(crate) fn refine_adaptive(&mut self, isolation: u32, consider_fvar: bool) {
        self.is_uniform = false;
        let mut mask = FeatureMask::new(true, consider_fvar);
        if mask.select_fvar_features {
            let any_nonlinear = self.levels[0].fvar.iter().any(|f| !f.is_linear);
            if !any_nonlinear {
                mask.select_fvar_features = false;
            }
        }
        for i in 1..=isolation as usize {
            frap_math::budget::check();
            let mut r = Refinement::default();
            self.select_feature_adaptive_components(i - 1, &mut r, &mask);
            if r.is_selection_empty() {
                break;
            }
            let mut child = Level::default();
            r.refine(&self.levels[i - 1], &mut child, true, false);
            self.levels.push(child);
            self.refinements.push(r);
        }
    }

    /// `selectFeatureAdaptiveComponents`
    fn select_feature_adaptive_components(
        &self,
        li: usize,
        r: &mut Refinement,
        mask: &FeatureMask,
    ) {
        let level = &self.levels[li];
        let nch = if mask.select_fvar_features {
            level.num_fvar_channels()
        } else {
            0
        };
        for f in 0..level.face_count {
            if self.has_holes && level.is_face_hole(f) {
                continue;
            }
            let mut select = does_face_have_features(level, f, mask);
            if !select && mask.select_fvar_features {
                for c in 0..nch {
                    if select {
                        break;
                    }
                    if !level.does_face_fvar_topology_match(f, c) {
                        select = does_face_have_distinct_fvar_features(level, f, mask, c);
                    }
                }
            }
            if select {
                r.select_face(level, f);
            }
        }
    }
}

/// `TopologyRefinerFactoryBase::prepareComponentTagsAndSharpness`
fn prepare_component_tags_and_sharpness(
    base: &mut Level,
    options: SdcOptions,
    has_irreg_faces: bool,
) {
    let sharpen_corner_verts = options.vtx_boundary == VtxBoundary::EdgeAndCorner;
    for e in 0..base.edge_count as usize {
        let boundary = base.num_edge_faces(e as Index) < 2;
        let t = &mut base.edge_tags[e];
        t.set_boundary(boundary);
        if t.boundary() || t.non_manifold() {
            base.edge_sharpness[e] = sdc::SHARPNESS_INFINITE;
        }
        let s = base.edge_sharpness[e];
        let inf = sdc::is_infinite(s);
        t.set_inf_sharp(inf);
        t.set_semi_sharp(sdc::is_sharp(s) && !inf);
    }
    let reg_interior = 4i32;
    let reg_boundary = reg_interior / 2;
    for v in 0..base.vert_count {
        let vu = v as usize;
        let v_edges = base.vertex_edges(v).to_vec();
        let nfaces = base.vertex_faces(v).len();
        let nedges = v_edges.len();
        let (mut boundary_n, mut inf_n, mut semi_n, mut nm_n) = (0, 0, 0, 0);
        for &e in &v_edges {
            let t = base.edge_tags[e as usize];
            boundary_n += t.boundary() as i32;
            inf_n += t.inf_sharp() as i32;
            semi_n += t.semi_sharp() as i32;
            nm_n += t.non_manifold() as i32;
        }
        let sharp_n = inf_n + semi_n;
        let topological_corner = nfaces == 1 && nedges == 2;
        let mut vtag = base.vert_tags[vu];
        let mut vs = base.vert_sharpness[vu];
        if topological_corner && sharpen_corner_verts {
            vs = sdc::SHARPNESS_INFINITE;
        } else if vtag.non_manifold() && !sdc::is_infinite(vs) {
            let is_nm_crease = nm_n == 2
                && boundary_n == 0
                && nfaces > nedges
                && base.test_vertex_non_manifold_crease(v);
            if !is_nm_crease {
                vs = sdc::SHARPNESS_INFINITE;
            }
        }
        base.vert_sharpness[vu] = vs;
        vtag.set_inf_sharp(sdc::is_infinite(vs));
        vtag.set_semi_sharp(sdc::is_semi_sharp(vs));
        vtag.set_semi_sharp_edges(semi_n > 0);
        vtag.set_rule(sdc::rule_from_count(vs, sharp_n));
        vtag.set_boundary(boundary_n > 0);
        vtag.set_corner(topological_corner && vtag.inf_sharp());
        let xord = if vtag.non_manifold() || vtag.corner() {
            false
        } else if vtag.boundary() {
            nfaces as i32 != reg_boundary
        } else {
            nfaces as i32 != reg_interior
        };
        vtag.set_xordinary(xord);
        vtag.set_incomplete(false);
        vtag.set_inf_sharp_edges(inf_n > 0);
        vtag.set_inf_sharp_crease(false);
        vtag.set_inf_irregular(vtag.inf_sharp() || vtag.inf_sharp_edges());
        if vtag.inf_sharp_edges() {
            let inf_rule = sdc::rule_from_count(if vtag.inf_sharp() { vs } else { 0.0 }, inf_n);
            if inf_rule == RULE_CREASE {
                vtag.set_inf_sharp_crease(true);
                if !vtag.xordinary() && !vtag.non_manifold() {
                    if vtag.boundary() {
                        vtag.set_inf_irregular(false);
                    } else {
                        let a = base.edge_tags[v_edges[0] as usize].inf_sharp();
                        let b = base.edge_tags[v_edges[2] as usize].inf_sharp();
                        vtag.set_inf_irregular(a != b);
                    }
                }
            } else if inf_rule == RULE_CORNER
                && inf_n as usize == nedges
                && (nedges > 2 || vtag.inf_sharp())
            {
                vtag.set_inf_irregular(false);
            }
        }
        if has_irreg_faces
            && base
                .vertex_faces(v)
                .iter()
                .any(|&f| base.num_face_vertices(f) != 4)
        {
            vtag.set_incid_irreg_face(true);
        }
        base.vert_tags[vu] = vtag;
    }
}

/// `doesInfSharpVTagHaveFeatures`
fn inf_sharp_vtag_has_features(t: VTag, m: &FeatureMask) -> bool {
    if t.inf_irregular() {
        if t.rule() & RULE_CORNER != 0 {
            return m.select_inf_sharp_irregular_corner;
        } else if t.rule() & RULE_CREASE != 0 {
            return if t.boundary() {
                m.select_xordinary_boundary
            } else {
                m.select_inf_sharp_irregular_crease
            };
        } else if t.rule() & RULE_DART != 0 {
            return m.select_inf_sharp_irregular_dart;
        }
    } else if t.boundary() {
        if t.rule() & RULE_CORNER != 0 {
            return if t.corner() {
                false
            } else {
                m.select_inf_sharp_regular_corner
            };
        } else {
            return false;
        }
    } else if t.rule() & RULE_CORNER != 0 {
        return m.select_inf_sharp_regular_corner;
    } else {
        return m.select_inf_sharp_regular_crease;
    }
    false
}

/// `doesInfSharpFaceHaveFeatures`
fn inf_sharp_face_has_features(comp: VTag, tags: &[VTag], m: &FeatureMask) -> bool {
    let at_least_one_smooth = comp.rule() & RULE_SMOOTH != 0;
    if tags.len() == 4 {
        if at_least_one_smooth {
            return inf_sharp_vtag_has_features(comp, m);
        } else if m.select_unisolated_interior_edge
            && tags.iter().any(|t| t.inf_sharp_edges() && !t.boundary())
        {
            return true;
        }
    } else if at_least_one_smooth && !comp.boundary() {
        return inf_sharp_vtag_has_features(comp, m);
    }
    tags.iter()
        .any(|&t| t.rule() & RULE_SMOOTH == 0 && inf_sharp_vtag_has_features(t, m))
}

/// `doesFaceHaveFeatures`
fn does_face_have_features(level: &Level, f: Index, m: &FeatureMask) -> bool {
    let fv = level.face_vertices(f);
    if fv.len() != 4 {
        return true;
    }
    let mut tags = [VTag::default(); 4];
    level.face_vtags(f, None, &mut tags);
    let comp = VTag::bitwise_or(&tags);
    if comp.incid_irreg_face() {
        return true;
    }
    if comp.incomplete() {
        return false;
    }
    if comp.non_manifold() && m.select_non_manifold {
        return true;
    }
    if comp.xordinary()
        && m.select_xordinary_interior
        && (comp.rule() == RULE_SMOOTH
            || (level.depth < 2
                && tags
                    .iter()
                    .any(|t| t.xordinary() && t.rule() == RULE_SMOOTH)))
    {
        return true;
    }
    if comp.rule() == RULE_SMOOTH {
        return false;
    }
    if comp.semi_sharp() || comp.semi_sharp_edges() {
        // `selectSemiSharpSingle && selectSemiSharpNonSingle` (no single-crease patches).
        return m.select_semi_sharp_single && m.select_semi_sharp_non_single;
    }
    if comp.inf_sharp() || comp.inf_sharp_edges() {
        return inf_sharp_face_has_features(comp, &tags, m);
    }
    false
}

/// `doesFaceHaveDistinctFaceVaryingFeatures`
fn does_face_have_distinct_fvar_features(
    level: &Level,
    f: Index,
    m: &FeatureMask,
    c: usize,
) -> bool {
    let fv = level.face_vertices(f);
    let tags: Vec<VTag> = fv
        .iter()
        .map(|&v| level.vertex_composite_fvar_vtag(v, c))
        .collect();
    let comp = VTag::bitwise_or(&tags);
    if comp.incomplete() {
        return false;
    }
    if comp.non_manifold() && m.select_non_manifold {
        return true;
    }
    if comp.xordinary() && m.select_xordinary_interior {
        return true;
    }
    inf_sharp_face_has_features(comp, &tags, m)
}
