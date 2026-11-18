// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_circle_add` geometry (ARCHITECTURE §6.1, tier T1).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
//! (`bmo_create_circle_exec`, `BM_mesh_calc_uvs_circle`) and editmesh_add.cc
//! (`add_primitive_circle_exec`: `fill_type` NOTHING/NGON/TRIFAN, no scale). NGON fill is the
//! fan's `dissolve_faces` (see [`super::cone`]).

use super::Matrix4;
use super::bm::Bm;
use super::bm::math::{invert_m4, mul_m4_v3, sin_cos_from_fraction};
use super::cone::{FillType, dissolve_fan};
use crate::mesh::Mesh;

/// Operator properties of `mesh.primitive_circle_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Circle {
    /// `vertices`.
    pub vertices: i32,
    /// `radius`.
    pub radius: f32,
    /// `fill_type` (default `NOTHING`).
    pub fill_type: FillType,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix (no scale for this operator).
    pub matrix: Matrix4,
}

impl Default for Circle {
    fn default() -> Self {
        Circle {
            vertices: 32,
            radius: 1.0,
            fill_type: FillType::Nothing,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

impl Circle {
    /// Element counts before building (`--max-elements` pre-check, [`super::Estimate`]).
    pub fn estimate(&self) -> Option<super::Estimate> {
        let n = u64::try_from(self.vertices.max(0)).ok()?;
        if n == 0 {
            return super::estimate(Some(0), Some(0), Some(0), Some(0));
        }
        match self.fill_type {
            FillType::Nothing => super::estimate(Some(n), Some(n), Some(0), Some(0)),
            FillType::Ngon => super::estimate(Some(n), Some(n), Some(1), Some(n)),
            FillType::Trifan => {
                super::estimate(n.checked_add(1), n.checked_mul(2), Some(n), n.checked_mul(3))
            }
        }
    }
}

/// The mesh of `mesh.primitive_circle_add(…)`.
pub fn circle(p: &Circle) -> Mesh {
    let cap_ends = p.fill_type != FillType::Nothing;
    let cap_tris = p.fill_type == FillType::Trifan;
    let mat = &p.matrix;
    let mut bm = Bm::new();
    let segs = p.vertices;
    if segs <= 0 {
        return bm.to_mesh("Circle", p.calc_uvs);
    }
    bm.circle = Some(super::trace::CircleTopology {
        segments: segs as u32,
        cap_ends,
        cap_tris,
    });
    let cent = cap_ends.then(|| bm.vert_create(mul_m4_v3(mat, [0.0; 3])));
    let mut ring: Vec<u32> = Vec::with_capacity(segs as usize);
    for a in 0..segs {
        let (s, c) = sin_cos_from_fraction(a, segs);
        let vec = [s * -p.radius, c * p.radius, 0.0];
        let v1 = bm.vert_create(mul_m4_v3(mat, vec));
        if let Some(&last) = ring.last() {
            bm.edge_create(v1, last);
            if let Some(c) = cent {
                bm.face_create(&[c, last, v1]);
            }
        }
        ring.push(v1);
    }
    let (first, last) = (ring[0], *ring.last().expect("segs > 0"));
    bm.edge_create(first, last);
    if let Some(c) = cent {
        bm.face_create(&[c, last, first]);
        if p.calc_uvs {
            // BM_mesh_calc_uvs_circle on the fan faces.
            let uv_scale = 0.5f32 / p.radius;
            let inv = invert_m4(mat);
            let positions: Vec<[f32; 3]> = bm.verts.slots.iter().map(|v| v.unwrap_or([0.0; 3])).collect();
            for slot in bm.faces.slots.iter_mut() {
                let Some(f) = slot.as_mut() else { continue };
                for (k, uv) in f.uvs.iter_mut().enumerate() {
                    let co = mul_m4_v3(&inv, positions[f.verts[k] as usize]);
                    *uv = [0.5 + uv_scale * co[0], 0.5 + uv_scale * co[1]];
                }
            }
        }
        if !cap_tris {
            dissolve_fan(&mut bm, c, &ring);
        }
    }
    bm.to_mesh("Circle", p.calc_uvs)
}
