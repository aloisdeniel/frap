// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.subdivide_edges` (Blender 5.2.4 `bmo_subdivide_edges`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.6 (Subdivide and loops).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.subdivide_edges(bmesh, edges=[], smooth=0.0, smooth_falloff='SMOOTH', fractal=0.0, along_normal=0.0, cuts=0, seed=0, custom_patterns={}, edge_percents={}, quad_corner_type='STRAIGHT_CUT', use_grid_fill=False, use_single_edge=False, use_only_quads=False, use_sphere=False, use_smooth_even=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId, FaceId};

/// Input slots of `subdivide_edges` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SubdivideEdgesIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `smooth`.
    pub smooth: f32,
    /// `smooth_falloff` (enum identifier).
    pub smooth_falloff: &'static str,
    /// `fractal`.
    pub fractal: f32,
    /// `along_normal`.
    pub along_normal: f32,
    /// `cuts`.
    pub cuts: i32,
    /// `seed`.
    pub seed: i32,
    /// `custom_patterns` (face -> pattern (unused by the UI)).
    pub custom_patterns: Vec<(FaceId, i32)>,
    /// `edge_percents` (edge -> cut factor).
    pub edge_percents: Vec<(EdgeId, f32)>,
    /// `quad_corner_type` (enum identifier).
    pub quad_corner_type: &'static str,
    /// `use_grid_fill`.
    pub use_grid_fill: bool,
    /// `use_single_edge`.
    pub use_single_edge: bool,
    /// `use_only_quads`.
    pub use_only_quads: bool,
    /// `use_sphere`.
    pub use_sphere: bool,
    /// `use_smooth_even`.
    pub use_smooth_even: bool,
}

impl Default for SubdivideEdgesIn {
    fn default() -> Self {
        SubdivideEdgesIn {
            edges: Vec::new(),
            smooth: 0.0,
            smooth_falloff: "SMOOTH",
            fractal: 0.0,
            along_normal: 0.0,
            cuts: 0,
            seed: 0,
            custom_patterns: Vec::new(),
            edge_percents: Vec::new(),
            quad_corner_type: "STRAIGHT_CUT",
            use_grid_fill: false,
            use_single_edge: false,
            use_only_quads: false,
            use_sphere: false,
            use_smooth_even: false,
        }
    }
}

/// Output slots of `subdivide_edges` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SubdivideEdgesOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `subdivide_edges` on `bm`.
pub fn subdivide_edges(bm: &mut BMesh, i: &SubdivideEdgesIn) -> Result<SubdivideEdgesOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.6: bmesh op subdivide_edges")
}
