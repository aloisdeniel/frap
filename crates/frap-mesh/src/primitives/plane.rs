// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_plane_add` geometry (ARCHITECTURE §6.1, tier T1): a 1×1 grid.
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_add.cc (`add_primitive_plane_exec`:
//! `create_grid x_segments=0 y_segments=0 size=size/2`, no scale; verified on 5.2.4 that the
//! operator's `scale` changes neither the mesh nor the object).

use super::Matrix4;
use super::grid::create_grid;
use crate::mesh::Mesh;

/// Operator properties of `mesh.primitive_plane_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Plane {
    /// `size` (full width).
    pub size: f32,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix (no scale for this operator).
    pub matrix: Matrix4,
}

impl Default for Plane {
    fn default() -> Self {
        Plane {
            size: 2.0,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

impl Plane {
    /// Element counts before building (`--max-elements` pre-check, [`super::Estimate`]).
    pub fn estimate(&self) -> Option<super::Estimate> {
        super::estimate(Some(4), Some(4), Some(1), Some(4))
    }
}

/// The mesh of `mesh.primitive_plane_add(…)`.
pub fn plane(p: &Plane) -> Mesh {
    create_grid("Plane", 0, 0, p.size / 2.0, p.calc_uvs, &p.matrix)
}
