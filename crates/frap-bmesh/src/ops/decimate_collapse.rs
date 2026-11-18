// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_decimate_collapse` (the DECIMATE modifier's `COLLAPSE` mode and `mesh.decimate`):
//! quadric edge collapse down to `factor` of the faces, optional vertex-group weights,
//! triangulation and symmetry (WP 4.5; stub created by M4 W0).
//!
//! Port target: blender/source/blender/bmesh/tools/bmesh_decimate_collapse.cc, Blender 5.2.4.

use crate::BMesh;

/// `BM_mesh_decimate_collapse` arguments.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DecimateCollapseIn {
    /// `factor` (`ratio`): the fraction of faces to keep.
    pub factor: f32,
    /// Per-vertex weights (`vertex_group` with `vertex_group_factor`), or `None`.
    pub vweights: Option<Vec<f32>>,
    /// `vertex_group_factor`.
    pub vweight_factor: f32,
    /// `use_collapse_triangulate`.
    pub do_triangulate: bool,
    /// `symmetry_axis` (`-1`: no symmetry).
    pub symmetry_axis: i32,
    /// Symmetry threshold (`BM_SYMMETRY_EPS`-scaled).
    pub symmetry_eps: f32,
}

impl Default for DecimateCollapseIn {
    fn default() -> Self {
        DecimateCollapseIn {
            factor: 1.0,
            vweights: None,
            vweight_factor: 1.0,
            do_triangulate: false,
            symmetry_axis: -1,
            symmetry_eps: 0.0,
        }
    }
}

/// `BM_mesh_decimate_collapse`.
pub fn decimate_collapse(bm: &mut BMesh, i: &DecimateCollapseIn) {
    let _ = (bm, i);
    todo!("WP 4.5: BM_mesh_decimate_collapse")
}
