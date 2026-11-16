// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_grid_add` geometry (ARCHITECTURE §6.1, tier T1).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc (`bmo_create_grid_exec`,
//! `BM_mesh_calc_uvs_grid`) and blender/source/blender/editors/mesh/editmesh_add.cc
//! (`add_primitive_grid_exec`: `size / 2`, no scale).

use super::Matrix4;
use super::bm::Bm;
use super::bm::math::mul_m4_v3;
use crate::mesh::Mesh;

/// Operator properties of `mesh.primitive_grid_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Grid {
    /// `x_subdivisions` (segments along X; values below 1 act as 1).
    pub x_subdivisions: i32,
    /// `y_subdivisions`.
    pub y_subdivisions: i32,
    /// `size` (full width).
    pub size: f32,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix (no scale for this operator).
    pub matrix: Matrix4,
}

impl Default for Grid {
    fn default() -> Self {
        Grid {
            x_subdivisions: 10,
            y_subdivisions: 10,
            size: 2.0,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

/// `bmo_create_grid_exec` with `dia = size / 2` (shared with the plane).
pub(crate) fn create_grid(name: &str, x_segments: i32, y_segments: i32, dia: f32, calc_uvs: bool, matrix: &Matrix4) -> Mesh {
    let xtot = x_segments.max(1) as u32;
    let ytot = y_segments.max(1) as u32;
    let xtot_inv2 = 2.0f32 / xtot as f32;
    let ytot_inv2 = 2.0f32 / ytot as f32;
    let mut bm = Bm::new();
    let mut varr = Vec::with_capacity(((xtot + 1) * (ytot + 1)) as usize);
    for y in 0..=ytot {
        let vy = ((y as f32 * ytot_inv2) - 1.0) * dia;
        for x in 0..=xtot {
            let vx = ((x as f32 * xtot_inv2) - 1.0) * dia;
            varr.push(bm.vert_create(mul_m4_v3(matrix, [vx, vy, 0.0])));
        }
    }
    let xy = |x: u32, y: u32| varr[(x + y * (xtot + 1)) as usize];
    for y in 1..=ytot {
        for x in 1..=xtot {
            bm.face_create(&[xy(x - 1, y - 1), xy(x, y - 1), xy(x, y), xy(x - 1, y)]);
        }
    }
    if calc_uvs {
        let dx = 1.0f32 / xtot as f32;
        let dy = 1.0f32 / ytot as f32;
        for (face_index, slot) in bm.faces.slots.iter_mut().enumerate() {
            let f = slot.as_mut().expect("no kills");
            let face_index = face_index as u32;
            let ix = face_index % xtot;
            let iy = face_index / xtot;
            f.uvs[0] = [ix as f32 * dx, iy as f32 * dy];
            f.uvs[1] = [(ix + 1) as f32 * dx, iy as f32 * dy];
            f.uvs[2] = [(ix + 1) as f32 * dx, (iy + 1) as f32 * dy];
            f.uvs[3] = [ix as f32 * dx, (iy + 1) as f32 * dy];
        }
    }
    bm.to_mesh(name, calc_uvs)
}

/// The mesh of `mesh.primitive_grid_add(…)`.
pub fn grid(p: &Grid) -> Mesh {
    create_grid("Grid", p.x_subdivisions, p.y_subdivisions, p.size / 2.0, p.calc_uvs, &p.matrix)
}
