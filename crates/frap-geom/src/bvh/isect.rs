// SPDX-License-Identifier: GPL-3.0-or-later
//! Watertight ray–triangle intersection (Woop, Benthin, Wald 2013), as the BVH ray casts with
//! `BVH_RAYCAST_WATERTIGHT` use it.
//!
//! Ported from blender/source/blender/blenlib/intern/math_geom.cc
//! (`isect_ray_tri_watertight_v3_precalc`, `isect_ray_tri_watertight_v3`), Blender 5.2.4,
//! GPL-2.0-or-later. `f32` throughout, in Blender's operation order.

use crate::normals::V3;

/// `IsectRayPrecalc`: the shear of a ray direction.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct IsectRayPrecalc {
    /// The dominant axis and the two others (winding-preserving).
    pub kx: usize,
    /// See [`IsectRayPrecalc::kx`].
    pub ky: usize,
    /// See [`IsectRayPrecalc::kx`].
    pub kz: usize,
    /// Shear constants.
    pub sx: f32,
    /// See [`IsectRayPrecalc::sx`].
    pub sy: f32,
    /// See [`IsectRayPrecalc::sx`].
    pub sz: f32,
}

/// `axis_dominant_v3_single`
fn axis_dominant(v: V3) -> usize {
    let (x, y, z) = (v[0].abs(), v[1].abs(), v[2].abs());
    if x > y {
        if x > z { 0 } else { 2 }
    } else if y > z {
        1
    } else {
        2
    }
}

impl IsectRayPrecalc {
    /// `isect_ray_tri_watertight_v3_precalc`
    pub fn new(dir: V3) -> IsectRayPrecalc {
        let kz = axis_dominant(dir);
        let mut kx = if kz != 2 { kz + 1 } else { 0 };
        let mut ky = if kx != 2 { kx + 1 } else { 0 };
        if dir[kz] < 0.0 {
            std::mem::swap(&mut kx, &mut ky);
        }
        let inv_dir_z = 1.0 / dir[kz];
        IsectRayPrecalc {
            kx,
            ky,
            kz,
            sx: dir[kx] * inv_dir_z,
            sy: dir[ky] * inv_dir_z,
            sz: inv_dir_z,
        }
    }
}

/// `isect_ray_tri_watertight_v3`: the distance `λ` along the ray (in units of the direction
/// the precalculation was made for) and the barycentric `uv` of the hit, or `None`.
pub fn isect_ray_tri_watertight_v3(
    origin: V3,
    pre: &IsectRayPrecalc,
    v0: V3,
    v1: V3,
    v2: V3,
) -> Option<(f32, [f32; 2])> {
    let (kx, ky, kz) = (pre.kx, pre.ky, pre.kz);
    let (sx, sy, sz) = (pre.sx, pre.sy, pre.sz);
    let a = [v0[0] - origin[0], v0[1] - origin[1], v0[2] - origin[2]];
    let b = [v1[0] - origin[0], v1[1] - origin[1], v1[2] - origin[2]];
    let c = [v2[0] - origin[0], v2[1] - origin[1], v2[2] - origin[2]];
    let (a_kx, a_ky, a_kz) = (a[kx], a[ky], a[kz]);
    let (b_kx, b_ky, b_kz) = (b[kx], b[ky], b[kz]);
    let (c_kx, c_ky, c_kz) = (c[kx], c[ky], c[kz]);
    let ax = a_kx - sx * a_kz;
    let ay = a_ky - sy * a_kz;
    let bx = b_kx - sx * b_kz;
    let by = b_ky - sy * b_kz;
    let cx = c_kx - sx * c_kz;
    let cy = c_ky - sy * c_kz;
    let u = cx * by - cy * bx;
    let v = ax * cy - ay * cx;
    let w = bx * ay - by * ax;
    if (u < 0.0 || v < 0.0 || w < 0.0) && (u > 0.0 || v > 0.0 || w > 0.0) {
        return None;
    }
    let det = u + v + w;
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let sign_det = det.to_bits() & 0x8000_0000;
    let t = (u * a_kz + v * b_kz + w * c_kz) * sz;
    // `xor_fl(t, sign_det)`
    let sign_t = f32::from_bits(t.to_bits() ^ sign_det);
    if sign_t < 0.0 {
        return None;
    }
    let inv_det = 1.0 / det;
    Some((t * inv_det, [u * inv_det, v * inv_det]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_and_misses() {
        let pre = IsectRayPrecalc::new([0.0, 0.0, -1.0]);
        let tri = ([-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]);
        let (l, uv) =
            isect_ray_tri_watertight_v3([0.0, 0.0, 2.0], &pre, tri.0, tri.1, tri.2).expect("hit");
        assert_eq!(l, 2.0);
        assert!(uv[0] >= 0.0 && uv[1] >= 0.0 && uv[0] + uv[1] <= 1.0);
        // Behind the origin.
        assert!(isect_ray_tri_watertight_v3([0.0, 0.0, -2.0], &pre, tri.0, tri.1, tri.2).is_none());
        // Beside the triangle.
        assert!(isect_ray_tri_watertight_v3([3.0, 0.0, 2.0], &pre, tri.0, tri.1, tri.2).is_none());
        // Both windings hit.
        assert!(isect_ray_tri_watertight_v3([0.0, 0.0, 2.0], &pre, tri.0, tri.2, tri.1).is_some());
    }

    #[test]
    fn shared_edges_are_watertight() {
        // A ray through the shared edge of two triangles hits at least one of them.
        let quad = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        for k in 0..50 {
            let t = k as f32 / 49.0 * 1.8 - 0.9;
            let o = [t, t, 3.0];
            let pre = IsectRayPrecalc::new([0.0, 0.0, -1.0]);
            let h1 = isect_ray_tri_watertight_v3(o, &pre, quad[0], quad[1], quad[2]);
            let h2 = isect_ray_tri_watertight_v3(o, &pre, quad[0], quad[2], quad[3]);
            assert!(h1.is_some() || h2.is_some(), "gap at {t}");
        }
    }
}
