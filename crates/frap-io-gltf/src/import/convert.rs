// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF (Y-up) → Blender (Z-up) conversions and the small amount of mathutils arithmetic the
//! importer does on transforms, in Blender's f32 precision.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/blender_gltf.py` (`set_convert_functions`,
//! Apache-2.0, The glTF-Blender-IO authors) and `blender/com/gltf2_blender_math.py`
//! (`scale_rot_swap_matrix`, `nearby_signed_perm_matrix`); `mul_qt_v3` from
//! blender/source/blender/blenlib/intern/math_rotation_c.cc (GPL-2.0-or-later).

use frap_math::glam::{Mat4, Vec4};
use frap_math::quat::{QuatWxyz, mul_qt_qtqt, quat_to_mat3};

/// `u`: Blender units per metre (`1 / scene.unit_settings.scale_length`), stored as the f32
/// mathutils multiplies by.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Units {
    /// `u` as the f64 Python computes.
    pub u: f64,
}

impl Units {
    /// From `scene.unit_settings.scale_length`.
    pub fn new(scale_length: f32) -> Units {
        Units {
            u: 1.0 / f64::from(scale_length),
        }
    }
    fn uf(self) -> f32 {
        self.u as f32
    }
    /// `convert_loc`: `u * Vector([x, -z, y])` (Vector stores f32; the product is f32).
    pub fn loc(self, x: [f64; 3]) -> [f32; 3] {
        let v = [x[0] as f32, (-x[2]) as f32, x[1] as f32];
        if self.u == 1.0 {
            v
        } else {
            let u = self.uf();
            [v[0] * u, v[1] * u, v[2] * u]
        }
    }
    /// `convert_locs_batch` on f32 data (numpy, in place).
    pub fn locs_batch(self, locs: &mut [[f32; 3]]) {
        for p in locs.iter_mut() {
            *p = [p[0], -p[2], p[1]];
            if self.u != 1.0 {
                // numpy: float32 array *= python float → float32 multiply (weak scalar).
                let u = self.uf();
                *p = [p[0] * u, p[1] * u, p[2] * u];
            }
        }
    }
    /// `convert_matrix` on a glTF column-major matrix, as a Blender (column-major glam) matrix.
    pub fn matrix(self, m: [f64; 16]) -> Mat4 {
        let u = self.u;
        let rows = [
            [m[0], -m[8], m[4], m[12] * u],
            [-m[2], m[10], -m[6], -m[14] * u],
            [m[1], -m[9], m[5], m[13] * u],
            [m[3] / u, -m[11] / u, m[7] / u, m[15]],
        ];
        let col = |c: usize| {
            Vec4::new(
                rows[0][c] as f32,
                rows[1][c] as f32,
                rows[2][c] as f32,
                rows[3][c] as f32,
            )
        };
        Mat4::from_cols(col(0), col(1), col(2), col(3))
    }
}

/// `convert_quat`: glTF `(x, y, z, w)` → Blender `(w, x, -z, y)`.
pub(crate) fn quat(q: [f64; 4]) -> QuatWxyz {
    [q[3] as f32, q[0] as f32, (-q[2]) as f32, q[1] as f32]
}

/// `convert_scale`: `(x, y, z)` → `(x, z, y)`.
pub(crate) fn scale(s: [f64; 3]) -> [f32; 3] {
    [s[0] as f32, s[2] as f32, s[1] as f32]
}

/// `convert_normals_batch`: `(x, y, z)` → `(x, -z, y)`.
pub(crate) fn normals_batch(ns: &mut [[f32; 3]]) {
    for n in ns.iter_mut() {
        *n = [n[0], -n[2], n[1]];
    }
}

/// `uvs_gltf_to_blender`: `v → 1 - v` (numpy: `v *= -1; v += 1`, f32).
pub(crate) fn uvs_batch(uvs: &mut [[f32; 2]]) {
    for uv in uvs.iter_mut() {
        uv[1] = -uv[1] + 1.0;
    }
}

/// `gltf.camera_correction`: `Quaternion((2**0.5 / 2, 2**0.5 / 2, 0, 0))`.
pub(crate) fn camera_correction() -> QuatWxyz {
    let h = (2f64.sqrt() / 2.0) as f32;
    [h, h, 0.0, 0.0]
}

/// `Quaternion @ Quaternion`.
pub(crate) fn qmul(a: QuatWxyz, b: QuatWxyz) -> QuatWxyz {
    mul_qt_qtqt(a, b)
}

/// `Quaternion.conjugated()`.
pub(crate) fn qconj(q: QuatWxyz) -> QuatWxyz {
    [q[0], -q[1], -q[2], -q[3]]
}

// Ported from blender/source/blender/blenlib/intern/math_rotation_c.cc (mul_qt_v3).
/// `Quaternion @ Vector` (`mul_qt_v3`).
pub(crate) fn qrot(q: QuatWxyz, r: [f32; 3]) -> [f32; 3] {
    let mut r = r;
    let t0 = -q[1] * r[0] - q[2] * r[1] - q[3] * r[2];
    let t1 = q[0] * r[0] + q[2] * r[2] - q[3] * r[1];
    let t2 = q[0] * r[1] + q[3] * r[0] - q[1] * r[2];
    r[2] = q[0] * r[2] + q[1] * r[1] - q[2] * r[0];
    r[0] = t1;
    r[1] = t2;
    let t1 = t0 * -q[1] + r[0] * q[0] - r[1] * q[3] + r[2] * q[2];
    let t2 = t0 * -q[2] + r[1] * q[0] - r[2] * q[1] + r[0] * q[3];
    r[2] = t0 * -q[3] + r[2] * q[0] - r[0] * q[2] + r[1] * q[1];
    r[0] = t1;
    r[1] = t2;
    r
}

/// `scale_rot_swap_matrix(rot) @ s`: the axis permutation that moves a scale across `rot`
/// (signs discarded). `rot` is snapped to the nearest signed permutation
/// (`nearby_signed_perm_matrix`), so the product only permutes `s`.
pub(crate) fn scale_rot_swap(rot: QuatWxyz, s: [f32; 3]) -> [f32; 3] {
    if rot == [1.0, 0.0, 0.0, 0.0] {
        return s;
    }
    // mathutils matrices are row-major: m[r][c]. glam is column-major: m.col(c)[r].
    let m = quat_to_mat3(rot);
    let row = |r: usize| [m.x_axis[r], m.y_axis[r], m.z_axis[r]];
    let (x, y) = (row(0), row(1));
    let (a, b, c) = (x[0].abs(), x[1].abs(), x[2].abs());
    let i = if a >= b && a >= c {
        0
    } else if b >= c {
        1
    } else {
        2
    };
    let (a, b) = (y[(i + 1) % 3].abs(), y[(i + 2) % 3].abs());
    let j = if a >= b { (i + 1) % 3 } else { (i + 2) % 3 };
    let k = 3 - i - j;
    // perm[r] = column of the ±1 in row r; the transpose maps s[r] to output[perm[r]].
    let perm = [i, j, k];
    let mut out = [0.0f32; 3];
    for r in 0..3 {
        out[perm[r]] = s[r];
    }
    out
}

/// `Vector.length` (mathutils sums in double).
pub(crate) fn length_f64(v: [f32; 3]) -> f64 {
    let d: f64 = v.iter().map(|&x| f64::from(x) * f64::from(x)).sum();
    d.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yup_to_zup() {
        let u = Units::new(1.0);
        assert_eq!(u.loc([1.0, 2.0, 3.0]), [1.0, -3.0, 2.0]);
        assert_eq!(quat([0.1, 0.2, 0.3, 0.9]), [0.9, 0.1, -0.3, 0.2]);
        assert_eq!(scale([1.0, 2.0, 3.0]), [1.0, 3.0, 2.0]);
        let mut n = [[0.0f32, 1.0, 0.0]];
        normals_batch(&mut n);
        assert_eq!(n, [[0.0, 0.0, 1.0]]);
        let mut uv = [[0.25f32, 0.25]];
        uvs_batch(&mut uv);
        assert_eq!(uv, [[0.25, 0.75]]);
    }

    #[test]
    fn unit_scale_divides() {
        let u = Units::new(2.0);
        assert_eq!(u.loc([2.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
        let mut p = [[2.0f32, 4.0, 6.0]];
        u.locs_batch(&mut p);
        assert_eq!(p, [[1.0, -3.0, 2.0]]);
    }

    #[test]
    fn matrix_matches_trs() {
        // translation (1, 2, 3) in glTF → (1, -3, 2) in Blender.
        let mut m = [0.0f64; 16];
        m[0] = 1.0;
        m[5] = 1.0;
        m[10] = 1.0;
        m[15] = 1.0;
        m[12] = 1.0;
        m[13] = 2.0;
        m[14] = 3.0;
        let b = Units::new(1.0).matrix(m);
        assert_eq!(b.w_axis.truncate().to_array(), [1.0, -3.0, 2.0]);
        assert_eq!(b.x_axis.truncate().to_array(), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn camera_correction_swaps_y_and_z_scale() {
        let c = camera_correction();
        assert_eq!(scale_rot_swap(c, [1.0, 2.0, 3.0]), [1.0, 3.0, 2.0]);
        assert_eq!(
            scale_rot_swap([1.0, 0.0, 0.0, 0.0], [1.0, 2.0, 3.0]),
            [1.0, 2.0, 3.0]
        );
        // Rotating +Y by 90° about X gives +Z.
        let r = qrot(c, [0.0, 1.0, 0.0]);
        assert!((r[2] - 1.0).abs() < 1e-6 && r[1].abs() < 1e-6, "{r:?}");
    }
}
