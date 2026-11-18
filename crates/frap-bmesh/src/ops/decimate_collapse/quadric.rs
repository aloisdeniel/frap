// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_quadric`: plane quadrics in double precision (the error metric of the edge-collapse
//! decimator).
//!
//! Ported from blender/source/blender/blenlib/intern/quadric.cc (Blender 5.2.4,
//! GPL-2.0-or-later).

/// `Quadric` (field order of `BLI_quadric.h`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct Quadric {
    a2: f64,
    ab: f64,
    ac: f64,
    ad: f64,
    b2: f64,
    bc: f64,
    bd: f64,
    c2: f64,
    cd: f64,
    d2: f64,
}

impl Quadric {
    /// `BLI_quadric_from_plane`.
    pub(crate) fn from_plane(v: [f64; 4]) -> Quadric {
        Quadric {
            a2: v[0] * v[0],
            b2: v[1] * v[1],
            c2: v[2] * v[2],
            ab: v[0] * v[1],
            ac: v[0] * v[2],
            bc: v[1] * v[2],
            ad: v[0] * v[3],
            bd: v[1] * v[3],
            cd: v[2] * v[3],
            d2: v[3] * v[3],
        }
    }

    /// `BLI_quadric_add_qu_qu`: `self += b`.
    pub(crate) fn add(&mut self, b: &Quadric) {
        self.a2 += b.a2;
        self.ab += b.ab;
        self.ac += b.ac;
        self.ad += b.ad;
        self.b2 += b.b2;
        self.bc += b.bc;
        self.bd += b.bd;
        self.c2 += b.c2;
        self.cd += b.cd;
        self.d2 += b.d2;
    }

    /// `BLI_quadric_add_qu_ququ`: `a + b`.
    pub(crate) fn sum(a: &Quadric, b: &Quadric) -> Quadric {
        let mut r = *a;
        r.add(b);
        r
    }

    /// `BLI_quadric_mul`.
    pub(crate) fn mul(&mut self, s: f64) {
        self.a2 *= s;
        self.ab *= s;
        self.ac *= s;
        self.ad *= s;
        self.b2 *= s;
        self.bc *= s;
        self.bd *= s;
        self.c2 *= s;
        self.cd *= s;
        self.d2 *= s;
    }

    /// `BLI_quadric_evaluate`.
    pub(crate) fn evaluate(&self, v: [f64; 3]) -> f64 {
        let v00 = v[0] * v[0];
        let v01 = v[0] * v[1];
        let v02 = v[0] * v[2];
        let v11 = v[1] * v[1];
        let v12 = v[1] * v[2];
        let v22 = v[2] * v[2];
        (self.a2 * v00)
            + (self.ab * 2.0 * v01)
            + (self.ac * 2.0 * v02)
            + (self.ad * 2.0 * v[0])
            + (self.b2 * v11)
            + (self.bc * 2.0 * v12)
            + (self.bd * 2.0 * v[1])
            + (self.c2 * v22)
            + (self.cd * 2.0 * v[2])
            + self.d2
    }

    /// `quadric_to_tensor_m3_inverse` (`m[col][row]` as Blender's `double[3][3]`).
    fn tensor_m3_inverse(&self, epsilon: f64) -> Option<[[f64; 3]; 3]> {
        let q = self;
        let det = q.a2 * (q.b2 * q.c2 - q.bc * q.bc) - q.ab * (q.ab * q.c2 - q.ac * q.bc)
            + q.ac * (q.ab * q.bc - q.ac * q.b2);
        if det.abs() > epsilon {
            let invdet = 1.0 / det;
            let mut m = [[0.0f64; 3]; 3];
            m[0][0] = (q.b2 * q.c2 - q.bc * q.bc) * invdet;
            m[1][0] = (q.bc * q.ac - q.ab * q.c2) * invdet;
            m[2][0] = (q.ab * q.bc - q.b2 * q.ac) * invdet;

            m[0][1] = (q.ac * q.bc - q.ab * q.c2) * invdet;
            m[1][1] = (q.a2 * q.c2 - q.ac * q.ac) * invdet;
            m[2][1] = (q.ab * q.ac - q.a2 * q.bc) * invdet;

            m[0][2] = (q.ab * q.bc - q.ac * q.b2) * invdet;
            m[1][2] = (q.ac * q.ab - q.a2 * q.bc) * invdet;
            m[2][2] = (q.a2 * q.b2 - q.ab * q.ab) * invdet;
            Some(m)
        } else {
            None
        }
    }

    /// `BLI_quadric_optimize`: the point minimising the quadric, when its tensor inverts.
    pub(crate) fn optimize(&self, epsilon: f64) -> Option<[f64; 3]> {
        let m = self.tensor_m3_inverse(epsilon)?;
        // `BLI_quadric_to_vector_v3`, `mul_m3_v3_db`, `negate_v3_db`.
        let a = [self.ad, self.bd, self.cd];
        let r = [
            m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
            m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
            m[0][2] * a[0] + m[1][2] * a[1] + m[2][2] * a[2],
        ];
        Some([-r[0], -r[1], -r[2]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_planes_meet_at_their_corner() {
        // x = 1, y = 2, z = -3: the quadric sum is minimal (zero) at the corner.
        let mut q = Quadric::from_plane([1.0, 0.0, 0.0, -1.0]);
        q.add(&Quadric::from_plane([0.0, 1.0, 0.0, -2.0]));
        q.add(&Quadric::from_plane([0.0, 0.0, 1.0, 3.0]));
        let p = q.optimize(1e-8).unwrap();
        assert!((p[0] - 1.0).abs() < 1e-12 && (p[1] - 2.0).abs() < 1e-12);
        assert!((p[2] + 3.0).abs() < 1e-12);
        assert!(q.evaluate(p).abs() < 1e-12);
        // Distance squared to the planes away from the corner.
        assert!((q.evaluate([0.0, 2.0, -3.0]) - 1.0).abs() < 1e-12);
        let mut d = q;
        d.mul(2.0);
        assert!((d.evaluate([0.0, 2.0, -3.0]) - 2.0).abs() < 1e-12);
        assert_eq!(Quadric::sum(&q, &q), d);
    }

    #[test]
    fn one_plane_does_not_optimize() {
        let q = Quadric::from_plane([0.0, 0.0, 1.0, 0.0]);
        assert!(q.optimize(1e-8).is_none());
    }
}
