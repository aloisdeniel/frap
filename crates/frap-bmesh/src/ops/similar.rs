// SPDX-License-Identifier: GPL-3.0-or-later
//! `similar`: the comparison core of `mesh.select_similar*` (Blender 5.2.4
//! `editmesh_select_similar.cc`, no `bmesh.ops` counterpart): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.4 (Selection).
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `similar` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `compare` (enum identifier).
    pub compare: &'static str,
    /// `threshold`.
    pub threshold: f32,
}

impl Default for SimilarIn {
    fn default() -> Self {
        SimilarIn {
            geom: Vec::new(),
            compare: "EQUAL",
            threshold: 0.0,
        }
    }
}

/// Output slots of `similar` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SimilarOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `similar` on `bm`.
pub fn similar(bm: &mut BMesh, i: &SimilarIn) -> Result<SimilarOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.4: bmesh op similar")
}
