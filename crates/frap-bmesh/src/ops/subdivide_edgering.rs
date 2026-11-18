// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.subdivide_edgering` (Blender 5.2.4 `bmo_subdivide_edgering`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.6 (Subdivide and loops).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.subdivide_edgering(bmesh, edges=[], interp_mode='LINEAR', smooth=0.0, cuts=0, profile_shape='SMOOTH', profile_shape_factor=0.0)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `subdivide_edgering` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SubdivideEdgeringIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `interp_mode` (enum identifier).
    pub interp_mode: &'static str,
    /// `smooth`.
    pub smooth: f32,
    /// `cuts`.
    pub cuts: i32,
    /// `profile_shape` (enum identifier).
    pub profile_shape: &'static str,
    /// `profile_shape_factor`.
    pub profile_shape_factor: f32,
}

impl Default for SubdivideEdgeringIn {
    fn default() -> Self {
        SubdivideEdgeringIn {
            edges: Vec::new(),
            interp_mode: "LINEAR",
            smooth: 0.0,
            cuts: 0,
            profile_shape: "SMOOTH",
            profile_shape_factor: 0.0,
        }
    }
}

/// Output slots of `subdivide_edgering` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SubdivideEdgeringOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `subdivide_edgering` on `bm`.
pub fn subdivide_edgering(bm: &mut BMesh, i: &SubdivideEdgeringIn) -> Result<SubdivideEdgeringOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.6: bmesh op subdivide_edgering")
}
