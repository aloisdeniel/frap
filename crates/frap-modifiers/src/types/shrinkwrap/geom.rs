// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BLI_math` geometry SHRINKWRAP calls, in Blender's operation order.
//!
//! Ported from blender/source/blender/blenlib/intern/math_geom.cc (`normal_tri_v3`,
//! `closest_on_tri_to_point_v3`, `interp_weights_tri_v3`, `barycentric_weights`),
//! blender/source/blender/blenlib/intern/math_solvers.cc (`BLI_newton3d_solve`) and
//! blender/source/blender/blenlib/intern/math_matrix_c.cc (`BLI_space_transform_*`,
//! `invert_m3`), Blender 5.2.4, GPL-2.0-or-later.

use frap_geom::normals::normalize_v3;

use super::super::warp::shared::inverted as invert_m4;
use super::super::weld::util::{M4, mul_m4_m4m4, mul_m4_v3};

pub(crate) type V3 = [f32; 3];
pub(crate) type M3 = [[f32; 3]; 3];

#[inline]
pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `madd_v3_v3v3fl`: `a + b·f`.
#[inline]
pub(crate) fn madd(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

#[inline]
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `len_squared_v3v3`
#[inline]
pub(crate) fn len_sq(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}

/// `len_manhattan_v3`
#[inline]
pub(crate) fn len_manhattan(a: V3) -> f32 {
    a[0].abs() + a[1].abs() + a[2].abs()
}

/// `normalize_v3` (the vector only).
#[inline]
pub(crate) fn normalized(a: V3) -> V3 {
    normalize_v3(a).0
}

/// `interp_v3_v3v3v3`
#[inline]
pub(crate) fn interp3(v1: V3, v2: V3, v3: V3, w: V3) -> V3 {
    [
        v1[0] * w[0] + v2[0] * w[1] + v3[0] * w[2],
        v1[1] * w[0] + v2[1] * w[1] + v3[1] * w[2],
        v1[2] * w[0] + v2[2] * w[1] + v3[2] * w[2],
    ]
}

/// `interp_v3_v3v3`
#[inline]
pub(crate) fn interp(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `normal_tri_v3`
pub(crate) fn normal_tri(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    normalized([
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ])
}

/// `closest_on_tri_to_point_v3` (Ericson, "Real-Time Collision Detection").
pub(crate) fn closest_on_tri_to_point(p: V3, v1: V3, v2: V3, v3: V3) -> V3 {
    let ab = sub(v2, v1);
    let ac = sub(v3, v1);
    let ap = sub(p, v1);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return v1;
    }
    let bp = sub(p, v2);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return v2;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let ab_squared = d1 - d3;
        if ab_squared == 0.0 {
            return v1;
        }
        return madd(v1, ab, d1 / ab_squared);
    }
    let cp = sub(p, v3);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return v3;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let ac_squared = d2 - d6;
        if ac_squared == 0.0 {
            return v1;
        }
        return madd(v1, ac, d2 / ac_squared);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let bc_squared = (d4 - d3) + (d5 - d6);
        if bc_squared == 0.0 {
            return v2;
        }
        let f = (d4 - d3) / bc_squared;
        let r = sub(v3, v2);
        return [r[0] * f + v2[0], r[1] * f + v2[1], r[2] * f + v2[2]];
    }
    // Inside the face.
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let acw = [ac[0] * w, ac[1] * w, ac[2] * w];
    let r = madd(v1, ab, v);
    [r[0] + acw[0], r[1] + acw[1], r[2] + acw[2]]
}

/// `axis_dominant_v3`: the two axes of the plane most facing `n`.
fn axis_dominant(n: V3) -> (usize, usize) {
    let (xn, yn, zn) = (n[0].abs(), n[1].abs(), n[2].abs());
    if zn >= xn && zn >= yn {
        (0, 1)
    } else if yn >= xn && yn >= zn {
        (0, 2)
    } else {
        (1, 2)
    }
}

/// `tri_signed_area`
fn tri_signed_area(v1: V3, v2: V3, v3: V3, i: usize, j: usize) -> f32 {
    0.5 * ((v1[i] - v2[i]) * (v2[j] - v3[j]) + (v1[j] - v2[j]) * (v3[i] - v2[i]))
}

/// `interp_weights_tri_v3`: barycentric weights of `co` (1/3 each for a degenerate
/// triangle).
pub(crate) fn interp_weights_tri(v1: V3, v2: V3, v3: V3, co: V3) -> V3 {
    let n = normal_tri(v1, v2, v3);
    let (i, j) = axis_dominant(n);
    let mut w = [
        tri_signed_area(v2, v3, co, i, j),
        tri_signed_area(v3, v1, co, i, j),
        tri_signed_area(v1, v2, co, i, j),
    ];
    let wtot = w[0] + w[1] + w[2];
    let f = 1.0 / wtot;
    for x in &mut w {
        *x *= f;
    }
    if w.iter().all(|x| x.is_finite()) {
        return w;
    }
    [1.0 / 3.0; 3]
}

/// `invert_m3` (in place, adjoint over determinant); `false` for a zero determinant.
pub(crate) fn invert_m3(m: &mut M3) -> bool {
    let (m00, m01, m02) = (m[0][0], m[0][1], m[0][2]);
    let (m10, m11, m12) = (m[1][0], m[1][1], m[1][2]);
    let (m20, m21, m22) = (m[2][0], m[2][1], m[2][2]);
    let mut r = [
        [
            m11 * m22 - m12 * m21,
            -m01 * m22 + m02 * m21,
            m01 * m12 - m02 * m11,
        ],
        [
            -m10 * m22 + m12 * m20,
            m00 * m22 - m02 * m20,
            -m00 * m12 + m02 * m10,
        ],
        [
            m10 * m21 - m11 * m20,
            -m00 * m21 + m01 * m20,
            m00 * m11 - m01 * m10,
        ],
    ];
    let det = m00 * (m11 * m22 - m12 * m21) - m10 * (m01 * m22 - m02 * m21)
        + m20 * (m01 * m12 - m02 * m11);
    if det == 0.0 {
        return false;
    }
    let inv = 1.0 / det;
    for c in &mut r {
        for v in c.iter_mut() {
            *v *= inv;
        }
    }
    *m = r;
    true
}

/// `mul_v3_m3v3`: `M · v` (`m[col][row]`).
pub(crate) fn mul_m3_v3(m: &M3, v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

/// A Newton problem of `BLI_newton3d_solve`: the deviation, its Jacobian and the step
/// correction.
pub(crate) trait Newton3d {
    /// `func_delta`
    fn delta(&mut self, x: V3) -> V3;
    /// `func_jacobian`
    fn jacobian(&mut self, x: V3) -> M3;
    /// `func_correction`: may change `step` and `x_next`; `false` aborts.
    fn correct(&mut self, x: V3, step: &mut V3, x_next: &mut V3) -> bool;
}

/// `BLI_newton3d_solve`: Newton's method with a line search; `(converged, x)`.
pub(crate) fn newton3d_solve(
    p: &mut dyn Newton3d,
    epsilon: f32,
    max_iterations: i32,
    x_init: V3,
) -> (bool, V3) {
    let epsilon = epsilon * epsilon;
    let mut x = x_init;
    let mut fdelta = p.delta(x);
    let mut fdeltav = dot(fdelta, fdelta);
    let mut i = 0;
    while i == 0 || (i < max_iterations && fdeltav > epsilon) {
        let mut jac = p.jacobian(x);
        if !invert_m3(&mut jac) {
            return (false, x);
        }
        let mut step = mul_m3_v3(&jac, fdelta);
        let mut x_next = sub(x, step);
        if !p.correct(x, &mut step, &mut x_next) {
            return (false, x);
        }
        fdelta = p.delta(x_next);
        let mut next_fdeltav = dot(fdelta, fdelta);
        // Line search correction.
        while next_fdeltav > fdeltav && next_fdeltav > epsilon {
            let g0 = fdeltav.sqrt();
            let g1 = next_fdeltav.sqrt();
            let g01 = -g0 / dot(step, step).sqrt();
            let det = 2.0 * (g1 - g0 - g01);
            let mut l = if det == 0.0 { 0.1 } else { -g01 / det };
            if l < 0.1 {
                l = 0.1;
            }
            for s in &mut step {
                *s *= l;
            }
            x_next = sub(x, step);
            fdelta = p.delta(x_next);
            next_fdeltav = dot(fdelta, fdelta);
        }
        x = x_next;
        fdeltav = next_fdeltav;
        i += 1;
    }
    (fdeltav <= epsilon, x)
}

/// `SpaceTransform`: from the deformed object's local space to the target's.
#[derive(Copy, Clone, Debug)]
pub(crate) struct SpaceTransform {
    pub local2target: M4,
    pub target2local: M4,
}

impl SpaceTransform {
    /// `BLI_space_transform_from_matrices(local, target)` (`BLI_SPACE_TRANSFORM_SETUP`).
    pub(crate) fn from_matrices(local: &M4, target: &M4) -> SpaceTransform {
        let itarget = invert_m4(target);
        let local2target = mul_m4_m4m4(&itarget, local);
        SpaceTransform {
            local2target,
            target2local: invert_m4(&local2target),
        }
    }

    /// `BLI_space_transform_apply`
    pub(crate) fn apply(&self, co: V3) -> V3 {
        mul_m4_v3(&self.local2target, co)
    }

    /// `BLI_space_transform_invert`
    pub(crate) fn invert(&self, co: V3) -> V3 {
        mul_m4_v3(&self.target2local, co)
    }

    /// `BLI_space_transform_apply_normal`: `mul_mat3_m4_v3(local2target)`, normalized.
    pub(crate) fn apply_normal(&self, no: V3) -> V3 {
        let m = &self.local2target;
        let [x, y, z] = no;
        normalized([
            x * m[0][0] + y * m[1][0] + m[2][0] * z,
            x * m[0][1] + y * m[1][1] + m[2][1] * z,
            x * m[0][2] + y * m[1][2] + m[2][2] * z,
        ])
    }

    /// `BLI_space_transform_invert_normal`: `mul_transposed_mat3_m4_v3(local2target)`,
    /// normalized.
    pub(crate) fn invert_normal(&self, no: V3) -> V3 {
        let m = &self.local2target;
        let [x, y, z] = no;
        normalized([
            x * m[0][0] + y * m[0][1] + z * m[0][2],
            x * m[1][0] + y * m[1][1] + z * m[1][2],
            x * m[2][0] + y * m[2][1] + z * m[2][2],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_point_regions() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(closest_on_tri_to_point([-1.0, -1.0, 0.0], a, b, c), a);
        assert_eq!(
            closest_on_tri_to_point([0.25, 0.25, 3.0], a, b, c),
            [0.25, 0.25, 0.0]
        );
        assert_eq!(
            closest_on_tri_to_point([0.5, -2.0, 0.0], a, b, c),
            [0.5, 0.0, 0.0]
        );
        let w = interp_weights_tri(a, b, c, [0.25, 0.25, 0.0]);
        assert!((w[0] - 0.5).abs() < 1e-6 && (w[1] - 0.25).abs() < 1e-6);
    }
}
