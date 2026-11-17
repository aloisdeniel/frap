// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_cube_add` geometry (ARCHITECTURE §10.4 step 3, target tier T1).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
//! (`bmo_create_cube_exec`, `BM_mesh_calc_uvs_cube`). Verified index by index against
//! Blender 5.2.4 in `tests/primitives_vs_blender.rs`.

use super::bm::Bm;
use super::bm::math::mul_m4_v3;
use super::{Matrix4, scale_matrix};
use crate::mesh::Mesh;

/// Face table of `bmo_create_cube_exec` ("rotation order set to match BM_mesh_elem_table_ensure").
const FACES: [[u32; 4]; 6] = [
    [0, 1, 3, 2],
    [2, 3, 7, 6],
    [6, 7, 5, 4],
    [4, 5, 1, 0],
    [2, 6, 4, 0],
    [7, 3, 1, 5],
];

/// Operator properties of `mesh.primitive_cube_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Cube {
    /// `size` (edge length; 0 gives a 1 m cube).
    pub size: f32,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix, operator `scale` included.
    pub matrix: Matrix4,
}

impl Default for Cube {
    fn default() -> Self {
        Cube {
            size: 2.0,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

/// The cube of `mesh.primitive_cube_add(size=…, calc_uvs=…)` with the placement scale baked in
/// (`scale` = (1,1,1) for the operator's default). M0 signature kept for its call sites.
pub fn cube(size: f32, calc_uvs: bool, scale: [f32; 3]) -> Mesh {
    cube_mesh(&Cube {
        size,
        calc_uvs,
        matrix: scale_matrix(scale),
    })
}

/// The mesh of `mesh.primitive_cube_add(…)`.
pub fn cube_mesh(p: &Cube) -> Mesh {
    // `size` is clamped to its hard range [0, 1e12] by argument conversion; a zero half-size
    // falls back to 0.5 (`if (!off) off = 0.5f;`), so size=0 and size=-1 give a 1 m cube
    // (verified in Blender 5.2.4).
    let mut off = p.size / 2.0;
    if off == 0.0 {
        off = 0.5;
    }
    let mut bm = Bm::new();
    let mut verts = [0u32; 8];
    let mut i = 0;
    for x in [-1i32, 1] {
        for y in [-1i32, 1] {
            for z in [-1i32, 1] {
                let v = [x as f32 * off, y as f32 * off, z as f32 * off];
                verts[i] = bm.vert_create(mul_m4_v3(&p.matrix, v));
                i += 1;
            }
        }
    }
    for f in FACES {
        bm.face_create(&f.map(|k| verts[k as usize]));
    }
    if p.calc_uvs {
        // BM_mesh_calc_uvs_cube: a vertical strip of four 0.25 squares at x = 0.375 for
        // faces 0..3, then faces 4 and 5 left and right of the third square.
        const W: f32 = 0.25;
        let starts = [
            (0.375f32, 0.0f32),
            (0.375, 0.25),
            (0.375, 0.5),
            (0.375, 0.75),
            (0.125, 0.5),
            (0.625, 0.5),
        ];
        for (slot, (x, y)) in bm.faces.slots.iter_mut().zip(starts) {
            let f = slot.as_mut().expect("no kills");
            f.uvs = vec![[x, y], [x + W, y], [x + W, y + W], [x, y + W]];
        }
    }
    bm.to_mesh("Cube", p.calc_uvs)
}
