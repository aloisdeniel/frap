// SPDX-License-Identifier: GPL-3.0-or-later
//! Eigen's `JacobiSVD` of a real 2 × 2 matrix (two-sided Jacobi with `real_2x2_jacobi_svd`,
//! singular values sorted in decreasing order, `ComputeFullU | ComputeFullV`), the only SVD
//! SLIM needs (`polar_svd` per triangle and the pin fit of `least_squares_relocator.cpp`).
//!
//! Ported from Eigen 3.4 (`Eigen/src/SVD/JacobiSVD.h`, `Eigen/src/Jacobi/Jacobi.h`), which
//! Blender 5.2.4 bundles in `extern/Eigen3` (MPL-2.0).

/// A 2 × 2 matrix, row-major (`m[row][col]`).
pub(crate) type Mat2 = [[f64; 2]; 2];

/// `JacobiRotation<double>`.
#[derive(Copy, Clone, Debug)]
struct Rot {
    c: f64,
    s: f64,
}

impl Rot {
    fn transpose(self) -> Rot {
        Rot {
            c: self.c,
            s: -self.s,
        }
    }

    fn mul(self, o: Rot) -> Rot {
        Rot {
            c: self.c * o.c - self.s * o.s,
            s: self.c * o.s + self.s * o.c,
        }
    }

    /// `makeJacobi(x, y, z)` for the symmetric 2 × 2 `[[x, y], [y, z]]`.
    fn make_jacobi(x: f64, y: f64, z: f64) -> Rot {
        let deno = 2.0 * y.abs();
        if deno < f64::MIN_POSITIVE {
            return Rot { c: 1.0, s: 0.0 };
        }
        let tau = (x - z) / deno;
        let w = (tau * tau + 1.0).sqrt();
        let t = if tau > 0.0 {
            1.0 / (tau + w)
        } else {
            1.0 / (tau - w)
        };
        let sign_t = if t > 0.0 { 1.0 } else { -1.0 };
        let n = 1.0 / (t * t + 1.0).sqrt();
        Rot {
            s: -sign_t * (y / y.abs()) * t.abs() * n,
            c: n,
        }
    }
}

/// `apply_rotation_in_the_plane(x, y, j)`.
fn rotate(x: &mut [f64; 2], y: &mut [f64; 2], j: Rot) {
    for i in 0..2 {
        let (xi, yi) = (x[i], y[i]);
        x[i] = j.c * xi + j.s * yi;
        y[i] = -j.s * xi + j.c * yi;
    }
}

fn apply_left(m: &mut Mat2, j: Rot) {
    let [mut r0, mut r1] = *m;
    rotate(&mut r0, &mut r1, j);
    *m = [r0, r1];
}

fn apply_right(m: &mut Mat2, j: Rot) {
    let mut c0 = [m[0][0], m[1][0]];
    let mut c1 = [m[0][1], m[1][1]];
    rotate(&mut c0, &mut c1, j.transpose());
    *m = [[c0[0], c1[0]], [c0[1], c1[1]]];
}

/// `real_2x2_jacobi_svd(matrix, 0, 1, &j_left, &j_right)`.
fn real_2x2_jacobi_svd(work: &Mat2) -> (Rot, Rot) {
    let mut m = *work;
    let t = m[0][0] + m[1][1];
    let d = m[1][0] - m[0][1];
    let rot1 = if d.abs() < f64::MIN_POSITIVE {
        Rot { c: 1.0, s: 0.0 }
    } else {
        let u = t / d;
        let tmp = (1.0 + u * u).sqrt();
        Rot {
            s: 1.0 / tmp,
            c: u / tmp,
        }
    };
    apply_left(&mut m, rot1);
    let j_right = Rot::make_jacobi(m[0][0], m[0][1], m[1][1]);
    let j_left = rot1.mul(j_right.transpose());
    (j_left, j_right)
}

/// `JacobiSVD<Matrix2d>(a, ComputeFullU | ComputeFullV)`: `(U, singular values, V)` with
/// `a = U diag(S) Vᵀ` and `S[0] >= S[1] >= 0`.
pub(crate) fn svd2(a: &Mat2) -> (Mat2, [f64; 2], Mat2) {
    let mut scale = a[0][0]
        .abs()
        .max(a[0][1].abs())
        .max(a[1][0].abs())
        .max(a[1][1].abs());
    if !scale.is_finite() {
        let nan = [[f64::NAN; 2]; 2];
        return (nan, [f64::NAN; 2], nan);
    }
    if scale == 0.0 {
        scale = 1.0;
    }
    let mut work = [
        [a[0][0] / scale, a[0][1] / scale],
        [a[1][0] / scale, a[1][1] / scale],
    ];
    let mut u: Mat2 = [[1.0, 0.0], [0.0, 1.0]];
    let mut v: Mat2 = [[1.0, 0.0], [0.0, 1.0]];
    let consider_as_zero = f64::MIN_POSITIVE;
    let precision = 2.0 * f64::EPSILON;
    let mut max_diag = work[0][0].abs().max(work[1][1].abs());
    let mut finished = false;
    let mut guard = 0;
    while !finished && guard < 64 {
        guard += 1;
        finished = true;
        let threshold = consider_as_zero.max(precision * max_diag);
        if work[1][0].abs().max(work[0][1].abs()) > threshold {
            finished = false;
            let (j_left, j_right) = real_2x2_jacobi_svd(&work);
            apply_left(&mut work, j_left);
            apply_right(&mut u, j_left.transpose());
            apply_right(&mut work, j_right);
            apply_right(&mut v, j_right);
            max_diag = max_diag.max(work[0][0].abs().max(work[1][1].abs()));
        }
    }
    let mut s = [0.0; 2];
    for i in 0..2 {
        let a = work[i][i];
        s[i] = a.abs();
        if a < 0.0 {
            u[0][i] = -u[0][i];
            u[1][i] = -u[1][i];
        }
    }
    s[0] *= scale;
    s[1] *= scale;
    if s[1] > s[0] {
        s.swap(0, 1);
        for m in [&mut u, &mut v] {
            for row in m.iter_mut() {
                row.swap(0, 1);
            }
        }
    }
    (u, s, v)
}

/// `a * b`.
pub(crate) fn mul(a: &Mat2, b: &Mat2) -> Mat2 {
    [
        [
            a[0][0] * b[0][0] + a[0][1] * b[1][0],
            a[0][0] * b[0][1] + a[0][1] * b[1][1],
        ],
        [
            a[1][0] * b[0][0] + a[1][1] * b[1][0],
            a[1][0] * b[0][1] + a[1][1] * b[1][1],
        ],
    ]
}

/// `aᵀ`.
pub(crate) fn transpose(a: &Mat2) -> Mat2 {
    [[a[0][0], a[1][0]], [a[0][1], a[1][1]]]
}

/// `determinant()`.
pub(crate) fn det(a: &Mat2) -> f64 {
    a[0][0] * a[1][1] - a[0][1] * a[1][0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(a: Mat2) {
        let (u, s, v) = svd2(&a);
        assert!(s[0] >= s[1] && s[1] >= 0.0);
        let d = [[s[0], 0.0], [0.0, s[1]]];
        let r = mul(&mul(&u, &d), &transpose(&v));
        for i in 0..2 {
            for j in 0..2 {
                assert!((r[i][j] - a[i][j]).abs() < 1e-12, "{a:?} -> {r:?}");
            }
        }
        let uu = mul(&transpose(&u), &u);
        let vv = mul(&transpose(&v), &v);
        for i in 0..2 {
            for j in 0..2 {
                let e = if i == j { 1.0 } else { 0.0 };
                assert!((uu[i][j] - e).abs() < 1e-12 && (vv[i][j] - e).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn reconstructs() {
        check([[1.0, 0.0], [0.0, 1.0]]);
        check([[0.3, -2.0], [1.5, 0.7]]);
        check([[2.0, 1.0], [4.0, 2.0]]);
        check([[0.0, 0.0], [0.0, 0.0]]);
        check([[-1.0, 0.0], [0.0, 3.0]]);
        check([[1e-9, 5.0], [-3.0, 1e-9]]);
        let (_, s, _) = svd2(&[[3.0, 0.0], [0.0, -4.0]]);
        assert_eq!(s, [4.0, 3.0]);
    }
}
