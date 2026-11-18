// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.beautify_fill` (Blender 5.2.4 `bmo_beautify_fill`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.beautify_fill(bmesh, faces=[], edges=[], use_restrict_tag=False, method='AREA')`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId, FaceId};

/// Input slots of `beautify_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct BeautifyFillIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_restrict_tag`.
    pub use_restrict_tag: bool,
    /// `method` (enum identifier).
    pub method: &'static str,
}

impl Default for BeautifyFillIn {
    fn default() -> Self {
        BeautifyFillIn {
            faces: Vec::new(),
            edges: Vec::new(),
            use_restrict_tag: false,
            method: "AREA",
        }
    }
}

/// Output slots of `beautify_fill` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BeautifyFillOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `beautify_fill` on `bm`.
pub fn beautify_fill(bm: &mut BMesh, i: &BeautifyFillIn) -> Result<BeautifyFillOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op beautify_fill")
}
