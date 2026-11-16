// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_ico_sphere_add` geometry (ARCHITECTURE §6.1, tier T1).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
//! (`bmo_create_icosphere_exec`, `icovert`, `icoface`, `icouvs`): the 20-face icosahedron, then
//! one `subdivide_edges` call with `cuts = 2^(subdivisions-1) - 1`, grid fill and sphere
//! smoothing (`bm/subdivide.rs`), then the creation matrix (operator `scale` included).

use super::Matrix4;
use super::bm::Bm;
use super::bm::math::mul_m4_v3;
use super::bm::subdivide::subdivide_sphere_tris;
use crate::mesh::Mesh;

#[rustfmt::skip]
const ICOVERT: [[f32; 3]; 12] = [
    [0.0, 0.0, -200.0],
    [144.72, -105.144, -89.443],
    [-55.277, -170.128, -89.443],
    [-178.885, 0.0, -89.443],
    [-55.277, 170.128, -89.443],
    [144.72, 105.144, -89.443],
    [55.277, -170.128, 89.443],
    [-144.72, -105.144, 89.443],
    [-144.72, 105.144, 89.443],
    [55.277, 170.128, 89.443],
    [178.885, 0.0, 89.443],
    [0.0, 0.0, 200.0],
];

#[rustfmt::skip]
const ICOFACE: [[usize; 3]; 20] = [
    [0, 1, 2], [1, 0, 5], [0, 2, 3], [0, 3, 4], [0, 4, 5], [1, 5, 10], [2, 1, 6],
    [3, 2, 7], [4, 3, 8], [5, 4, 9], [1, 10, 6], [2, 6, 7], [3, 7, 8], [4, 8, 9],
    [5, 9, 10], [6, 10, 11], [7, 6, 11], [8, 7, 11], [9, 8, 11], [10, 9, 11],
];

#[rustfmt::skip]
const ICOUVS: [[f32; 2]; 60] = [
    [0.181819, 0.000000], [0.272728, 0.157461], [0.090910, 0.157461], [0.272728, 0.157461],
    [0.363637, 0.000000], [0.454546, 0.157461], [0.909091, 0.000000], [1.000000, 0.157461],
    [0.818182, 0.157461], [0.727273, 0.000000], [0.818182, 0.157461], [0.636364, 0.157461],
    [0.545455, 0.000000], [0.636364, 0.157461], [0.454546, 0.157461], [0.272728, 0.157461],
    [0.454546, 0.157461], [0.363637, 0.314921], [0.090910, 0.157461], [0.272728, 0.157461],
    [0.181819, 0.314921], [0.818182, 0.157461], [1.000000, 0.157461], [0.909091, 0.314921],
    [0.636364, 0.157461], [0.818182, 0.157461], [0.727273, 0.314921], [0.454546, 0.157461],
    [0.636364, 0.157461], [0.545455, 0.314921], [0.272728, 0.157461], [0.363637, 0.314921],
    [0.181819, 0.314921], [0.090910, 0.157461], [0.181819, 0.314921], [0.000000, 0.314921],
    [0.818182, 0.157461], [0.909091, 0.314921], [0.727273, 0.314921], [0.636364, 0.157461],
    [0.727273, 0.314921], [0.545455, 0.314921], [0.454546, 0.157461], [0.545455, 0.314921],
    [0.363637, 0.314921], [0.181819, 0.314921], [0.363637, 0.314921], [0.272728, 0.472382],
    [0.000000, 0.314921], [0.181819, 0.314921], [0.090910, 0.472382], [0.727273, 0.314921],
    [0.909091, 0.314921], [0.818182, 0.472382], [0.545455, 0.314921], [0.727273, 0.314921],
    [0.636364, 0.472382], [0.363637, 0.314921], [0.545455, 0.314921], [0.454546, 0.472382],
];

/// Operator properties of `mesh.primitive_ico_sphere_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct IcoSphere {
    /// `subdivisions` (1..=10).
    pub subdivisions: i32,
    /// `radius`.
    pub radius: f32,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix, operator `scale` included.
    pub matrix: Matrix4,
}

impl Default for IcoSphere {
    fn default() -> Self {
        IcoSphere {
            subdivisions: 2,
            radius: 1.0,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

/// The mesh of `mesh.primitive_ico_sphere_add(…)`.
pub fn ico_sphere(p: &IcoSphere) -> Mesh {
    let rad = p.radius;
    let rad_div = rad / 200.0;
    let mut bm = Bm::new();
    let eva: Vec<u32> = ICOVERT
        .iter()
        .map(|v| bm.vert_create([rad_div * v[0], rad_div * v[1], rad_div * v[2]]))
        .collect();
    for (a, f) in ICOFACE.iter().enumerate() {
        let fi = bm.face_create(&[eva[f[0]], eva[f[1]], eva[f[2]]]);
        if p.calc_uvs {
            bm.faces.get_mut(fi).expect("live").uvs = ICOUVS[a * 3..a * 3 + 3].to_vec();
        }
    }
    if p.subdivisions > 1 {
        let cuts = (1usize << (p.subdivisions - 1).min(30)) - 1;
        subdivide_sphere_tris(&mut bm, cuts, rad);
    }
    for slot in bm.verts.slots.iter_mut().flatten() {
        *slot = mul_m4_v3(&p.matrix, *slot);
    }
    bm.to_mesh("Icosphere", p.calc_uvs)
}
