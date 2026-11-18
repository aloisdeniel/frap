// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.duplicate` (Blender 5.2.4 `bmo_duplicate`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.1 (BMesh core).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.duplicate(bmesh, geom=[], dest=None, use_select_history=False, use_edge_flip_from_face=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `duplicate` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DuplicateIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `dest` (another BMesh (unsupported: always this one)).
    pub dest: Option<()>,
    /// `use_select_history`.
    pub use_select_history: bool,
    /// `use_edge_flip_from_face`.
    pub use_edge_flip_from_face: bool,
}

/// Output slots of `duplicate` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DuplicateOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `duplicate` on `bm`.
pub fn duplicate(bm: &mut BMesh, i: &DuplicateIn) -> Result<DuplicateOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.1: bmesh op duplicate")
}
