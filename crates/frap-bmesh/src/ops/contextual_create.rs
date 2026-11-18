// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.contextual_create` (Blender 5.2.4 `bmo_contextual_create`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.contextual_create(bmesh, geom=[], mat_nr=0, use_smooth=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `contextual_create` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContextualCreateIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `mat_nr`.
    pub mat_nr: i32,
    /// `use_smooth`.
    pub use_smooth: bool,
}

/// Output slots of `contextual_create` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ContextualCreateOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `contextual_create` on `bm`.
pub fn contextual_create(bm: &mut BMesh, i: &ContextualCreateIn) -> Result<ContextualCreateOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op contextual_create")
}
