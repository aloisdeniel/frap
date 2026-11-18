// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.bridge_loops` (Blender 5.2.4 `bmo_bridge_loops`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.bridge_loops(bmesh, edges=[], use_pairs=False, use_cyclic=False, use_merge=False, merge_factor=0.0, twist_offset=0)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `bridge_loops` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeLoopsIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_pairs`.
    pub use_pairs: bool,
    /// `use_cyclic`.
    pub use_cyclic: bool,
    /// `use_merge`.
    pub use_merge: bool,
    /// `merge_factor`.
    pub merge_factor: f32,
    /// `twist_offset`.
    pub twist_offset: i32,
}

/// Output slots of `bridge_loops` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BridgeLoopsOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `bridge_loops` on `bm`.
pub fn bridge_loops(bm: &mut BMesh, i: &BridgeLoopsIn) -> Result<BridgeLoopsOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op bridge_loops")
}
