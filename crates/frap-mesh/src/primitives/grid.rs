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

impl Grid {
    /// Element counts before building (`--max-elements` pre-check, [`super::Estimate`]).
    pub fn estimate(&self) -> Option<super::Estimate> {
        let x = u64::try_from(self.x_subdivisions.max(1)).ok()?;
        let y = u64::try_from(self.y_subdivisions.max(1)).ok()?;
        let f = x.checked_mul(y)?;
        super::estimate(
            (x + 1).checked_mul(y + 1),
            x.checked_mul(y + 1)?.checked_add(y.checked_mul(x + 1)?),
            Some(f),
            f.checked_mul(4),
        )
    }
}

/// `bmo_create_grid_exec` with `dia = size / 2` (shared with the plane).
pub(crate) fn create_grid(name: &str, x_segments: i32, y_segments: i32, dia: f32, calc_uvs: bool, matrix: &Matrix4) -> Mesh {
    // `usize` (not Blender's `uint`) so the vertex count cannot wrap; callers bound the counts
    // first (`Grid::estimate`, `--max-elements`), the pools refuse more than `u32` elements.
    let xtot = x_segments.max(1) as usize;
    let ytot = y_segments.max(1) as usize;
    let xtot_inv2 = 2.0f32 / xtot as f32;
    let ytot_inv2 = 2.0f32 / ytot as f32;
    let mut bm = Bm::new();
    let mut varr = Vec::with_capacity((xtot + 1).saturating_mul(ytot + 1));
    for y in 0..=ytot {
        let vy = ((y as f32 * ytot_inv2) - 1.0) * dia;
        for x in 0..=xtot {
            let vx = ((x as f32 * xtot_inv2) - 1.0) * dia;
            varr.push(bm.vert_create(mul_m4_v3(matrix, [vx, vy, 0.0])));
        }
    }
    let xy = |x: usize, y: usize| varr[x + y * (xtot + 1)];
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
