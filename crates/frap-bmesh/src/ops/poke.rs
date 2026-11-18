// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.poke` (Blender 5.2.4 `bmo_poke`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.poke(bmesh, faces=[], offset=0.0, center_mode='MEAN_WEIGHTED', use_relative_offset=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `poke` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct PokeIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `offset`.
    pub offset: f32,
    /// `center_mode` (enum identifier).
    pub center_mode: &'static str,
    /// `use_relative_offset`.
    pub use_relative_offset: bool,
}

impl Default for PokeIn {
    fn default() -> Self {
        PokeIn {
            faces: Vec::new(),
            offset: 0.0,
            center_mode: "MEAN_WEIGHTED",
            use_relative_offset: false,
        }
    }
}

/// Output slots of `poke` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PokeOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `poke` on `bm`.
pub fn poke(bm: &mut BMesh, i: &PokeIn) -> Result<PokeOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op poke")
}
