// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_cube_add` geometry (ARCHITECTURE §10.4 step 3, target tier T1).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
//! (`bmo_create_cube_exec`, `BM_mesh_calc_uvs_cube`). Verified index by index against
//! Blender 5.2.4 in `tests/cube_vs_blender.rs` (fixtures: spec/m0/primitives/*.json).

use std::sync::Arc;

use crate::mesh::{AttrData, Attribute, Domain, Mesh, SHARP_FACE};

/// Face table of `bmo_create_cube_exec` ("rotation order set to match BM_mesh_elem_table_ensure").
const FACES: [[i32; 4]; 6] = [
    [0, 1, 3, 2],
    [2, 3, 7, 6],
    [6, 7, 5, 4],
    [4, 5, 1, 0],
    [2, 6, 4, 0],
    [7, 3, 1, 5],
];

/// Default UV map name (`DATA_("UVMap")`).
pub const UV_MAP_NAME: &str = "UVMap";

/// The cube of `mesh.primitive_cube_add(size=…, calc_uvs=…)`, in object space with the
/// placement scale already baked in (`scale` = (1,1,1) for the operator's default).
pub fn cube(size: f32, calc_uvs: bool, scale: [f32; 3]) -> Mesh {
    // `size` is clamped to its hard range [0, 1e12] by argument conversion; a zero half-size
    // falls back to 0.5 (`if (!off) off = 0.5f;`), so size=0 and size=-1 give a 1 m cube
    // (verified in Blender 5.2.4, fixtures primitive_cube_add__size0 / __size_negative).
    let off = if size / 2.0 == 0.0 { 0.5 } else { size / 2.0 };
    let mut positions = Vec::with_capacity(8);
    for x in [-1.0f32, 1.0] {
        for y in [-1.0f32, 1.0] {
            for z in [-1.0f32, 1.0] {
                positions.push([off * x * scale[0], off * y * scale[1], off * z * scale[2]]);
            }
        }
    }
    let faces: Vec<&[i32]> = FACES.iter().map(|f| f.as_slice()).collect();
    let mut mesh = Mesh::from_faces_bmesh_order("Cube", positions, &faces);
    mesh.attributes_mut().insert(Attribute {
        name: SHARP_FACE.into(),
        domain: Domain::Face,
        data: AttrData::Bool(Arc::new(vec![true; 6])),
    });
    if calc_uvs {
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
        let mut uvs = Vec::with_capacity(24);
        for (x, y) in starts {
            uvs.extend_from_slice(&[[x, y], [x + W, y], [x + W, y + W], [x, y + W]]);
        }
        mesh.attributes_mut().insert(Attribute {
            name: UV_MAP_NAME.into(),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(uvs)),
        });
        mesh.uv_active = Some(UV_MAP_NAME.into());
        mesh.uv_active_render = Some(UV_MAP_NAME.into());
    }
    mesh
}
