// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.split` (Blender 5.2.4 `bmo_split`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.split(bmesh, geom=[], dest=None, use_only_faces=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `split` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SplitIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `dest` (another BMesh (unsupported: always this one)).
    pub dest: Option<()>,
    /// `use_only_faces`.
    pub use_only_faces: bool,
}

/// Output slots of `split` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SplitOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `split` on `bm`.
pub fn split(bm: &mut BMesh, i: &SplitIn) -> Result<SplitOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op split")
}
