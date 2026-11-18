// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.spin` (Blender 5.2.4 `bmo_spin`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.spin(bmesh, geom=[], cent=Vector(), axis=Vector(), dvec=Vector(), angle=0.0, space=Matrix(), steps=0, use_merge=False, use_normal_flip=False, use_duplicate=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};
use crate::IDENTITY;

/// Input slots of `spin` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SpinIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `cent`.
    pub cent: [f32; 3],
    /// `axis`.
    pub axis: [f32; 3],
    /// `dvec`.
    pub dvec: [f32; 3],
    /// `angle`.
    pub angle: f32,
    /// `space` (Blender layout `m[col][row]`).
    pub space: [[f32; 4]; 4],
    /// `steps`.
    pub steps: i32,
    /// `use_merge`.
    pub use_merge: bool,
    /// `use_normal_flip`.
    pub use_normal_flip: bool,
    /// `use_duplicate`.
    pub use_duplicate: bool,
}

impl Default for SpinIn {
    fn default() -> Self {
        SpinIn {
            geom: Vec::new(),
            cent: [0.0; 3],
            axis: [0.0; 3],
            dvec: [0.0; 3],
            angle: 0.0,
            space: IDENTITY,
            steps: 0,
            use_merge: false,
            use_normal_flip: false,
            use_duplicate: false,
        }
    }
}

/// Output slots of `spin` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SpinOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `spin` on `bm`.
pub fn spin(bm: &mut BMesh, i: &SpinIn) -> Result<SpinOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op spin")
}
