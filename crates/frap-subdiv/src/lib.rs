// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-subdiv` (layer L3): the [`Subdivider`] trait (CONTRACT, M4 W0), frap's own
//! Catmull-Clark with creases and face-varying UVs ([`catmull_clark`]), the optional
//! `opensubdiv-rs` backend ([`osd`], feature `osd`, off until the M4 gate, ARCHITECTURE §3.6) and
//! the pass that reorders the output like Blender's `subdiv_mesh` ([`layout`]).
//!
//! The crate sits below `frap-mesh` (L4), so it works on plain arrays: [`SubdivTopology`] is
//! the base mesh in `Mesh`'s struct-of-arrays layout, [`SubdivResult`] the subdivided mesh plus
//! the interpolation weights the SUBSURF evaluator (`frap-modifiers/src/types/subsurf.rs`)
//! applies to every other attribute. Blender's reference is
//! `blender/source/blender/blenkernel/intern/subdiv*.cc` over OpenSubdiv (`intern/opensubdiv`).
//!
//! Ownership (M4): WP 4.4 owns this crate (`crates/frap-subdiv/**`) after W0; the signatures
//! below are the contract `frap-eval`'s `ModEvalCx::subdivider` hands out. WP 4.4 added
//! [`SubdivResult::vert_sources`] / [`SubdivResult::corner_sources`] (Blender's two-stage
//! attribute interpolation, which flattened weights cannot reproduce for integer and boolean
//! attributes) and [`SubdivResult::identity`].
//!
//! Internals: `sdc` (Catmull-Clark masks and creasing rules), `vtr` (levels and
//! uniform refinement), `fvar` (face-varying channels), `primvar` (interpolation and limit),
//! all ported from OpenSubdiv as Blender links it.
#![deny(unsafe_code)]

pub mod catmull_clark;
mod fvar;
pub mod layout;
#[cfg(feature = "osd")]
pub mod osd;
mod primvar;
mod sdc;
mod vtr;

pub use layout::{DataSource, PosSource};

use smallvec::SmallVec;

/// `Subdiv` vertex boundary interpolation (`SubsurfModifier.boundary_smooth`:
/// `ALL` → [`VtxBoundary::EdgeOnly`], `PRESERVE_CORNERS` → [`VtxBoundary::EdgeAndCorner`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VtxBoundary {
    /// `SUBDIV_VTX_BOUNDARY_NONE`
    None,
    /// `SUBDIV_VTX_BOUNDARY_EDGE_ONLY`
    #[default]
    EdgeOnly,
    /// `SUBDIV_VTX_BOUNDARY_EDGE_AND_CORNER`
    EdgeAndCorner,
}

/// `Subdiv` face-varying (UV) linear interpolation (`OpenSubdiv`'s `FVarLinearInterpolation`),
/// from `SubsurfModifier.uv_smooth` by [`FVarLinear::from_uv_smooth`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FVarLinear {
    /// `FVAR_LINEAR_NONE`: smooth everywhere (`uv_smooth='SMOOTH_ALL'`).
    None,
    /// `FVAR_LINEAR_CORNERS_ONLY` (`PRESERVE_CORNERS`).
    CornersOnly,
    /// `FVAR_LINEAR_CORNERS_PLUS1` (`PRESERVE_CORNERS_AND_JUNCTIONS`).
    CornersPlusOne,
    /// `FVAR_LINEAR_CORNERS_PLUS2` (`PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE`).
    CornersPlusTwo,
    /// `FVAR_LINEAR_BOUNDARIES` (`PRESERVE_BOUNDARIES`, the 5.2 default).
    #[default]
    Boundaries,
    /// `FVAR_LINEAR_ALL`: every UV linear (`uv_smooth='NONE'`).
    All,
}

impl FVarLinear {
    /// `bke::subdiv::fvar_interpolation_from_uv_smooth` (unknown identifiers: `All`, as
    /// Blender's fallback).
    pub fn from_uv_smooth(uv_smooth: &str) -> FVarLinear {
        match uv_smooth {
            "PRESERVE_CORNERS" => FVarLinear::CornersOnly,
            "PRESERVE_CORNERS_AND_JUNCTIONS" => FVarLinear::CornersPlusOne,
            "PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE" => FVarLinear::CornersPlusTwo,
            "PRESERVE_BOUNDARIES" => FVarLinear::Boundaries,
            "SMOOTH_ALL" => FVarLinear::None,
            _ => FVarLinear::All,
        }
    }
}

impl VtxBoundary {
    /// `bke::subdiv::vtx_boundary_interpolation_from_subsurf`
    pub fn from_boundary_smooth(boundary_smooth: &str) -> VtxBoundary {
        match boundary_smooth {
            "PRESERVE_CORNERS" => VtxBoundary::EdgeAndCorner,
            _ => VtxBoundary::EdgeOnly,
        }
    }
}

/// `blender::bke::subdiv::Settings` (`BKE_subsurf_modifier_settings_init` fills it from the
/// modifier).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SubdivSettings {
    /// `subdivision_type == 'SIMPLE'`
    pub is_simple: bool,
    /// Cycles' adaptive subdivision (`use_adaptive_subdivision`); not evaluated by frap.
    /// (Blender's own `Settings::is_adaptive` is `use_limit_surface` here.)
    pub is_adaptive: bool,
    /// Levels (`levels` in the viewport, `render_levels` in a render): the output resolution
    /// is `(1 << level) + 1` points per coarse edge.
    pub level: u32,
    /// `use_creases`
    pub use_creases: bool,
    /// `boundary_smooth`
    pub vtx_boundary: VtxBoundary,
    /// `uv_smooth`
    pub fvar_linear: FVarLinear,
    /// `use_limit_surface` (positions on the limit surface; Blender's adaptive refinement).
    pub use_limit_surface: bool,
    /// `quality` (OpenSubdiv's isolation level for the limit evaluation).
    pub quality: u32,
}

impl Default for SubdivSettings {
    /// The settings of a fresh SUBSURF modifier at its viewport level 1.
    fn default() -> Self {
        SubdivSettings {
            is_simple: false,
            is_adaptive: false,
            level: 1,
            use_creases: true,
            vtx_boundary: VtxBoundary::EdgeOnly,
            fvar_linear: FVarLinear::Boundaries,
            use_limit_surface: true,
            quality: 3,
        }
    }
}

/// The base mesh, in `frap_mesh::Mesh`'s layout (positions per vertex, edges as vertex pairs,
/// faces as offsets into the corner arrays).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SubdivTopology<'a> {
    /// Vertex positions.
    pub positions: &'a [[f32; 3]],
    /// Edge vertex pairs.
    pub edges: &'a [[u32; 2]],
    /// Face offsets into the corner arrays (`faces + 1` entries).
    pub face_offsets: &'a [u32],
    /// Vertex of each corner.
    pub corner_verts: &'a [u32],
    /// Edge of each corner (the edge from this corner to the next one).
    pub corner_edges: &'a [u32],
    /// `crease_edge` (0..1), when present and `use_creases`.
    pub edge_creases: Option<&'a [f32]>,
    /// `crease_vert` (0..1), when present and `use_creases`.
    pub vert_creases: Option<&'a [f32]>,
    /// Face-varying channels (UV maps), one value per corner each.
    pub uv_layers: Vec<&'a [[f32; 2]]>,
}

impl<'a> SubdivTopology<'a> {
    /// A topology without creases or UV maps.
    pub fn new(
        positions: &'a [[f32; 3]],
        edges: &'a [[u32; 2]],
        face_offsets: &'a [u32],
        corner_verts: &'a [u32],
        corner_edges: &'a [u32],
    ) -> Self {
        SubdivTopology {
            positions,
            edges,
            face_offsets,
            corner_verts,
            corner_edges,
            edge_creases: None,
            vert_creases: None,
            uv_layers: Vec::new(),
        }
    }
}

/// An output element as a weighted sum of base elements (vertex weights for vertex-domain
/// attributes, corner weights for corner-domain attributes).
pub type Weights = SmallVec<[(u32, f32); 4]>;

/// The subdivided mesh and how to interpolate the base mesh's attributes onto it.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SubdivResult {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Edge vertex pairs.
    pub edges: Vec<[u32; 2]>,
    /// Face offsets (`faces + 1` entries).
    pub face_offsets: Vec<u32>,
    /// Vertex of each corner.
    pub corner_verts: Vec<u32>,
    /// Edge of each corner.
    pub corner_edges: Vec<u32>,
    /// Face-varying channels, in the order of [`SubdivTopology::uv_layers`].
    pub uv_layers: Vec<Vec<[f32; 2]>>,
    /// Per output vertex: weights over base vertices.
    pub vert_weights: Vec<Weights>,
    /// Per output edge: the base edge it lies on (`None`: an inner edge).
    pub edge_origin: Vec<Option<u32>>,
    /// Per output face: the base face it comes from.
    pub face_origin: Vec<u32>,
    /// Per output corner: weights over base corners.
    pub corner_weights: Vec<Weights>,
    /// Per output edge: its crease. Blender copies `crease_edge` like any edge attribute: the
    /// edges along a coarse edge keep its value, inner edges get 0 (no decay).
    pub edge_creases: Option<Vec<f32>>,
    /// Per output vertex: where Blender takes its attributes from (`subdiv_mesh.cc`), for
    /// exact integer/boolean mixing; [`SubdivResult::vert_weights`] is its linear expansion.
    pub vert_sources: Vec<DataSource>,
    /// Per output corner: where Blender takes its attributes from;
    /// [`SubdivResult::corner_weights`] is its linear expansion.
    pub corner_sources: Vec<DataSource>,
}

impl SubdivResult {
    /// The base mesh unchanged (level 0), every element its own source.
    pub fn identity(topo: &SubdivTopology<'_>) -> SubdivResult {
        let one = |i: usize| -> Weights { smallvec::smallvec![(i as u32, 1.0)] };
        let nf = topo.face_offsets.len().saturating_sub(1);
        SubdivResult {
            positions: topo.positions.to_vec(),
            edges: topo.edges.to_vec(),
            face_offsets: topo.face_offsets.to_vec(),
            corner_verts: topo.corner_verts.to_vec(),
            corner_edges: topo.corner_edges.to_vec(),
            uv_layers: topo.uv_layers.iter().map(|l| l.to_vec()).collect(),
            vert_weights: (0..topo.positions.len()).map(one).collect(),
            edge_origin: (0..topo.edges.len() as u32).map(Some).collect(),
            face_origin: (0..nf as u32).collect(),
            corner_weights: (0..topo.corner_verts.len()).map(one).collect(),
            edge_creases: topo.edge_creases.map(|c| c.to_vec()),
            vert_sources: (0..topo.positions.len() as u32)
                .map(DataSource::Copy)
                .collect(),
            corner_sources: (0..topo.corner_verts.len() as u32)
                .map(DataSource::Copy)
                .collect(),
        }
    }
}

/// Why subdivision failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SubdivError {
    /// The topology cannot be refined (`Subdiv` creation failed).
    #[error("{0}")]
    Topology(String),
    /// The result would exceed `--max-elements` (§12).
    #[error("{0}")]
    TooLarge(String),
    /// The backend is not available in this build.
    #[error("{0}")]
    Unavailable(String),
}

/// A subdivision backend (CONTRACT, ARCHITECTURE §3.2, §7.2): frap's own Catmull-Clark, or
/// OpenSubdiv behind the `osd` feature. Implementations are stateless.
pub trait Subdivider: Send + Sync {
    /// Backend name (`"frap"`, `"opensubdiv"`).
    fn name(&self) -> &'static str;
    /// Subdivides `topo` with `settings` (`level == 0` returns the base mesh unchanged).
    fn subdivide(
        &self,
        topo: &SubdivTopology<'_>,
        settings: &SubdivSettings,
    ) -> Result<SubdivResult, SubdivError>;
}

/// The backend `frap-eval` hands to the SUBSURF evaluator: OpenSubdiv with the `osd` feature,
/// frap's own Catmull-Clark otherwise.
pub fn default_subdivider() -> &'static dyn Subdivider {
    #[cfg(feature = "osd")]
    {
        &osd::OpenSubdiv
    }
    #[cfg(not(feature = "osd"))]
    {
        &catmull_clark::CatmullClark
    }
}
