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
//! below are the contract `frap-eval`'s `ModEvalCx::subdivider` hands out. WP 4.4 may add
//! fields to the `#[non_exhaustive]` structs and items to the modules.
#![deny(unsafe_code)]

pub mod catmull_clark;
pub mod layout;
#[cfg(feature = "osd")]
pub mod osd;

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

/// `Subdiv` face-varying (UV) linear interpolation (`SubsurfModifier.uv_smooth`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FVarLinear {
    /// `NONE` (smooth everywhere)
    None,
    /// `PRESERVE_CORNERS` (the 5.2 default)
    #[default]
    CornersOnly,
    /// `PRESERVE_CORNERS_AND_JUNCTIONS`
    CornersPlusOne,
    /// `PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE`
    CornersPlusTwo,
    /// `PRESERVE_BOUNDARIES`
    Boundaries,
    /// `SMOOTH_ALL` is `None`; `ALL` keeps every UV linear.
    All,
}

/// `blender::bke::subdiv::Settings` (`BKE_subsurf_modifier_settings_init` fills it from the
/// modifier).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SubdivSettings {
    /// `subdivision_type == 'SIMPLE'`
    pub is_simple: bool,
    /// Adaptive subdivision (Cycles only; always false in frap).
    pub is_adaptive: bool,
    /// Levels (`levels` in the viewport, `render_levels` in a render).
    pub level: u32,
    /// `use_creases`
    pub use_creases: bool,
    /// `boundary_smooth`
    pub vtx_boundary: VtxBoundary,
    /// `uv_smooth`
    pub fvar_linear: FVarLinear,
    /// `use_limit_surface` (positions on the limit surface).
    pub use_limit_surface: bool,
    /// `quality` (OpenSubdiv's adaptive refinement level for the limit evaluation).
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
            fvar_linear: FVarLinear::CornersOnly,
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
    /// Per output edge: its crease (subdivided creases decay like Blender's).
    pub edge_creases: Option<Vec<f32>>,
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
