// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_decimate_unsubdivide` (the DECIMATE modifier's `UNSUBDIV` mode): un-subdivide every
//! vertex `iterations` times. M4 W0 wrote the wrapper (an exact port over WP 2.6's
//! `decimate_unsubdivide_ex`); WP 4.5 owns this file.
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_decimate_unsubdivide.cc
//! (`BM_mesh_decimate_unsubdivide`), Blender 5.2.4, GPL-2.0-or-later.

use crate::BMesh;
use crate::ops::unsubdivide::decimate_unsubdivide_ex;

/// `BM_mesh_decimate_unsubdivide(bm, iterations)`.
pub fn decimate_unsubdivide(bm: &mut BMesh, iterations: i32) {
    decimate_unsubdivide_ex(bm, iterations, false);
}
