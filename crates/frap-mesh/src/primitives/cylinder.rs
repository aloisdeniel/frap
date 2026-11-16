// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_cylinder_add` geometry (ARCHITECTURE §6.1, tier T1): a cone with equal
//! radii.
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_add.cc
//! (`add_primitive_cylinder_exec`: `create_cone radius1=radius radius2=radius`, the creation
//! matrix includes `scale`); the geometry is [`super::cone`]'s.

use super::Matrix4;
use super::cone::{FillType, create_cone};
use crate::mesh::Mesh;

/// Operator properties of `mesh.primitive_cylinder_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Cylinder {
    /// `vertices`.
    pub vertices: i32,
    /// `radius`.
    pub radius: f32,
    /// `depth`.
    pub depth: f32,
    /// `end_fill_type`.
    pub end_fill_type: FillType,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix, operator `scale` included.
    pub matrix: Matrix4,
}

impl Default for Cylinder {
    fn default() -> Self {
        Cylinder {
            vertices: 32,
            radius: 1.0,
            depth: 2.0,
            end_fill_type: FillType::Ngon,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

/// The mesh of `mesh.primitive_cylinder_add(…)`.
pub fn cylinder(p: &Cylinder) -> Mesh {
    create_cone(
        "Cylinder",
        p.vertices,
        p.radius,
        p.radius,
        p.depth,
        p.end_fill_type,
        p.calc_uvs,
        &p.matrix,
    )
}
