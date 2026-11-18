// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.find_doubles` (Blender 5.2.4 `bmo_find_doubles`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.find_doubles(bmesh, verts=[], keep_verts=[], use_connected=False, dist=0.0)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `find_doubles` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FindDoublesIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `keep_verts`.
    pub keep_verts: Vec<VertId>,
    /// `use_connected`.
    pub use_connected: bool,
    /// `dist`.
    pub dist: f32,
}

/// Output slots of `find_doubles` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FindDoublesOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `find_doubles` on `bm`.
pub fn find_doubles(bm: &mut BMesh, i: &FindDoublesIn) -> Result<FindDoublesOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op find_doubles")
}
