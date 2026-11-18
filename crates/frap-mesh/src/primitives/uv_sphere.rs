// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_uv_sphere_add` geometry (ARCHITECTURE §6.1).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
//! (`bmo_create_uvsphere_exec`, `BM_mesh_calc_uvs_sphere`): a half-circle profile of
//! `ring_count + 1` vertices (`sin_cos_from_fraction`), extruded and rotated `segments` times by
//! `-2π / segments` about Z around the last profile point (the extrusions copy vertices in
//! profile order; the rotation is `transform_pivot_set_m4` + `mul_m4_v3`, so each ring is the
//! previous ring rotated in float), then `remove_doubles` with a third of the shortest segment
//! as threshold (`frap_geom::kdtree::find_doubles_by_distance`, Blender's kd-tree clustering:
//! which copy of a seam or pole vertex survives depends on the exact float positions), then the
//! creation matrix (operator `scale` included).
//!
//! **Vertex order and positions are Blender's (T1). Edge and face order are not a property of
//! Blender 5.2:** the extrusion iterates a pointer-keyed `GHash` (`boundary_map.out`) and the
//! weld rebuilds faces in that order, so three runs of the same Blender build give different
//! edge/face orders (verified on 5.2.4; vertex order and the topology itself are stable). frap
//! uses one fixed order: per segment, the bottom triangle, the quads from the top down, the top
//! triangle, with Blender's corner order per face; edges in face-creation order. Goldens
//! compare UV-sphere edges and faces canonically (T2) and vertices index by index.

use super::Matrix4;
use super::bm::Bm;
use super::bm::math::{
    axis_angle_normalized_to_mat3, len_v3, m4_from_m3, mul_m4_v3, mul_v3_m3v3,
    sin_cos_from_fraction, transform_pivot_set_m4,
};
use crate::mesh::Mesh;
use frap_geom::kdtree::find_doubles_by_distance;

/// Operator properties of `mesh.primitive_uv_sphere_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UvSphere {
    /// `segments` (values below 1 act as 1).
    pub segments: i32,
    /// `ring_count` (values below 1 act as 1).
    pub ring_count: i32,
    /// `radius`.
    pub radius: f32,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix, operator `scale` included.
    pub matrix: Matrix4,
}

impl Default for UvSphere {
    fn default() -> Self {
        UvSphere {
            segments: 32,
            ring_count: 16,
            radius: 1.0,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

impl UvSphere {
    /// Element counts before building (`--max-elements` pre-check, [`super::Estimate`]).
    pub fn estimate(&self) -> Option<super::Estimate> {
        let s = u64::try_from(self.segments.max(1)).ok()?;
        let r = u64::try_from(self.ring_count.max(1)).ok()?;
        if r < 2 {
            // A single ring collapses to its two poles.
            return super::estimate(Some(2), Some(0), Some(0), Some(0));
        }
        // `s` quads per inner ring band, a triangle fan at each pole.
        let quads = s.checked_mul(r - 2)?;
        super::estimate(
            s.checked_mul(r - 1)?.checked_add(2),
            s.checked_mul(2 * r - 1),
            s.checked_mul(r),
            quads.checked_mul(4)?.checked_add(s.checked_mul(6)?),
        )
    }
}

/// The mesh of `mesh.primitive_uv_sphere_add(…)`.
pub fn uv_sphere(p: &UvSphere) -> Mesh {
    let rad = p.radius;
    let seg = p.segments.max(1) as usize;
    let tot = p.ring_count.max(1) as usize;
    let phid = std::f32::consts::PI / tot as f32;

    // Profile (a = 0 top … tot bottom), then `seg` rotated copies.
    let mut profile = Vec::with_capacity(tot + 1);
    let mut vec = [0.0f32; 3];
    for a in 0..=tot {
        let (sin_phi, cos_phi) = sin_cos_from_fraction(a as i32, 2 * tot as i32);
        vec = [0.0, rad * sin_phi, rad * cos_phi];
        profile.push(vec);
    }
    let angle = (-(std::f64::consts::PI * 2.0) / seg as f64) as f32;
    let cmat = axis_angle_normalized_to_mat3([0.0, 0.0, 1.0], angle);
    let rot = transform_pivot_set_m4(&m4_from_m3(&cmat), vec);
    let ring_len = tot + 1;
    let mut coords: Vec<[f32; 3]> = Vec::with_capacity(ring_len * (seg + 1));
    coords.extend_from_slice(&profile);
    for k in 1..=seg {
        for a in 0..ring_len {
            let prev = coords[(k - 1) * ring_len + a];
            coords.push(mul_m4_v3(&rot, prev));
        }
    }

    // remove_doubles with a third of the shortest segment length.
    let len = 2.0 * rad * libm::sinf(phid / 2.0);
    let v0 = [rad * libm::sinf(phid), 0.0, rad * libm::cosf(phid)];
    let v1 = mul_v3_m3v3(&cmat, v0);
    let len2 = len_v3([v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]]);
    let dist = len.min(len2) / 3.0;
    let target: Vec<i32> = find_doubles_by_distance(&coords, dist, None)
        .unwrap_or_else(|| (0..coords.len() as i32).collect());

    let mut bm = Bm::new();
    let mut slot = vec![u32::MAX; coords.len()];
    for (i, co) in coords.iter().enumerate() {
        if target[i] == i as i32 {
            slot[i] = bm.vert_create(*co);
        }
    }
    let vid = |k: usize, a: usize| -> u32 { slot[target[k * ring_len + a] as usize] };

    let mut face_list: Vec<Vec<u32>> = Vec::with_capacity(seg * tot);
    for k in 0..seg {
        let mut push = |verts: Vec<u32>| {
            let mut uniq = verts.clone();
            uniq.sort_unstable();
            uniq.dedup();
            if uniq.len() == verts.len() && verts.len() >= 3 {
                face_list.push(verts);
            }
        };
        if tot >= 2 {
            push(vec![vid(k, tot), vid(k, tot - 1), vid(k + 1, tot - 1)]);
            for a in 1..tot - 1 {
                push(vec![
                    vid(k, a + 1),
                    vid(k, a),
                    vid(k + 1, a),
                    vid(k + 1, a + 1),
                ]);
            }
            push(vec![vid(k, 1), vid(k, 0), vid(k + 1, 1)]);
        }
    }
    for f in &face_list {
        bm.face_create(f);
    }
    if p.calc_uvs {
        calc_uvs_sphere(&mut bm);
    }
    for co in bm.verts.slots.iter_mut().flatten() {
        *co = mul_m4_v3(&p.matrix, *co);
    }
    bm.to_mesh("Sphere", p.calc_uvs)
}

// Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
// (bm_mesh_calc_uvs_sphere_face, BM_mesh_calc_uvs_sphere).
fn calc_uvs_sphere(bm: &mut Bm) {
    let pi = std::f32::consts::PI;
    let positions: Vec<[f32; 3]> = bm
        .verts
        .slots
        .iter()
        .map(|v| v.unwrap_or([0.0; 3]))
        .collect();
    for f in bm.faces.slots.iter_mut().flatten() {
        let n = f.verts.len();
        let (mut avgx, mut avgy) = (0.0f32, 0.0f32);
        if n == 3 {
            for &v in &f.verts {
                avgx += positions[v as usize][0];
                avgy += positions[v as usize][1];
            }
        }
        avgx /= 3.0;
        avgy /= 3.0;
        for (k, &v) in f.verts.iter().enumerate() {
            let co = positions[v as usize];
            let (x, y, z) = (co[0], co[1], co[2]);
            let len = len_v3(co);
            let mut theta = if n == 3 && x.abs() < 0.0001 && y.abs() < 0.0001 {
                libm::atan2f(avgy, avgx)
            } else {
                libm::atan2f(y, x)
            };
            if (theta - pi).abs() < 0.0001 {
                theta = -std::f64::consts::PI as f32;
            }
            let phi = libm::acosf((z / len).clamp(-1.0, 1.0));
            f.uvs[k] = [0.5 + theta / (pi * 2.0), 1.0 - phi / pi];
        }
        let mut max_i = 0;
        for k in 1..n {
            if f.uvs[k][0] > f.uvs[max_i][0] {
                max_i = k;
            }
        }
        for k in 0..n {
            if k != max_i {
                let dx = f.uvs[max_i][0] - f.uvs[k][0];
                if dx > 0.5 {
                    f.uvs[k][0] += 1.0;
                }
            }
        }
    }
    let mut minx = 1.0f32;
    for f in bm.faces.slots.iter().flatten() {
        for uv in &f.uvs {
            minx = uv[0].min(minx);
        }
    }
    for f in bm.faces.slots.iter_mut().flatten() {
        for uv in f.uvs.iter_mut() {
            uv[0] -= minx;
        }
    }
}
