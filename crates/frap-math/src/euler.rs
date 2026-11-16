// SPDX-License-Identifier: GPL-3.0-or-later
//! Euler rotations with Blender's conventions.
//!
//! Provenance: Blender 5.2.4, `source/blender/blenlib/intern/math_rotation_c.cc`
//! (GPL-2.0-or-later): `eulO_to_mat3`, `eulO_to_quat`, `mat3_normalized_to_eulo2`,
//! `mat3_normalized_to_eulO`, `mat3_normalized_to_compatible_eulO`, `compatible_eul`,
//! `quat_to_eulO`, `quat_to_compatible_eulO`. The arbitrary-order code is Ken Shoemake's
//! "Euler Angle Conversion" (Graphics Gems IV) as adapted by Blender. Operation order and the
//! f32/f64 mix are kept exactly; transcendentals come from `libm` (rule 2.2.12).
//!
//! Matrices are `glam::Mat3` (column-major, like Blender's `float m[3][3]` = `m[col][row]`).
//! `XYZ` means `R = Rz · Ry · Rx` (Blender's `eul_to_mat3`), never `glam::EulerRot`.

use glam::Mat3;
use serde::{Deserialize, Serialize};

use crate::matrix::{M3, from_m3, normalize_m3, to_m3};
use crate::quat::quat_to_mat3;

/// `EULER_HYPOT_EPSILON`
const EULER_HYPOT_EPSILON: f32 = 0.000_037_5;

/// `Object.rotation_mode` Euler orders (Blender identifiers `'XYZ'`, `'XZY'`, …).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EulerOrder {
    /// `'XYZ'` (Blender default)
    #[default]
    XYZ,
    /// `'XZY'`
    XZY,
    /// `'YXZ'`
    YXZ,
    /// `'YZX'`
    YZX,
    /// `'ZXY'`
    ZXY,
    /// `'ZYX'`
    ZYX,
}

impl EulerOrder {
    /// Every order, in Blender's `eEulerRotationOrders` order.
    pub const ALL: [EulerOrder; 6] = [
        EulerOrder::XYZ,
        EulerOrder::XZY,
        EulerOrder::YXZ,
        EulerOrder::YZX,
        EulerOrder::ZXY,
        EulerOrder::ZYX,
    ];

    /// Blender's identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            EulerOrder::XYZ => "XYZ",
            EulerOrder::XZY => "XZY",
            EulerOrder::YXZ => "YXZ",
            EulerOrder::YZX => "YZX",
            EulerOrder::ZXY => "ZXY",
            EulerOrder::ZYX => "ZYX",
        }
    }
    /// Parses Blender's identifier.
    pub fn from_blender(s: &str) -> Option<Self> {
        Some(match s {
            "XYZ" => EulerOrder::XYZ,
            "XZY" => EulerOrder::XZY,
            "YXZ" => EulerOrder::YXZ,
            "YZX" => EulerOrder::YZX,
            "ZXY" => EulerOrder::ZXY,
            "ZYX" => EulerOrder::ZYX,
            _ => return None,
        })
    }
    /// DNA `rotmode` value (`ROT_MODE_XYZ = 1` … `ROT_MODE_ZYX = 6`).
    pub const fn rotmode(self) -> i16 {
        match self {
            EulerOrder::XYZ => 1,
            EulerOrder::XZY => 2,
            EulerOrder::YXZ => 3,
            EulerOrder::YZX => 4,
            EulerOrder::ZXY => 5,
            EulerOrder::ZYX => 6,
        }
    }
    /// `rotOrders[]`: axis permutation `(i, j, k)` and parity.
    const fn info(self) -> ([usize; 3], bool) {
        match self {
            EulerOrder::XYZ => ([0, 1, 2], false),
            EulerOrder::XZY => ([0, 2, 1], true),
            EulerOrder::YXZ => ([1, 0, 2], true),
            EulerOrder::YZX => ([1, 2, 0], false),
            EulerOrder::ZXY => ([2, 0, 1], false),
            EulerOrder::ZYX => ([2, 1, 0], true),
        }
    }
}

/// An Euler rotation (radians) with its order (`mathutils.Euler`).
#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Euler {
    /// Angles in radians, X Y Z.
    pub angles: [f32; 3],
    /// Rotation order.
    pub order: EulerOrder,
}

impl Euler {
    /// New Euler.
    pub const fn new(angles: [f32; 3], order: EulerOrder) -> Self {
        Euler { angles, order }
    }
    /// `Euler.to_matrix()` (`eulO_to_mat3`).
    pub fn to_matrix(&self) -> Mat3 {
        eul_o_to_mat3(self.angles, self.order)
    }
    /// `Euler.to_quaternion()` (`eulO_to_quat`), w-first.
    pub fn to_quaternion(&self) -> [f32; 4] {
        eul_o_to_quat(self.angles, self.order)
    }
    /// `Matrix.to_euler(order)`: normalizes the columns, then `mat3_normalized_to_eulO`.
    pub fn from_matrix(m: &Mat3, order: EulerOrder) -> Self {
        Euler::new(mat3_to_eul_o(m, order), order)
    }
    /// `Matrix.to_euler(order, compat)`.
    pub fn from_matrix_compatible(m: &Mat3, order: EulerOrder, compat: [f32; 3]) -> Self {
        Euler::new(mat3_to_compatible_eul_o(m, compat, order), order)
    }
    /// `Euler.make_compatible(other)` (`compatible_eul`).
    pub fn make_compatible(&mut self, other: [f32; 3]) {
        self.angles = compatible_eul(self.angles, other);
    }
}

/// `eulO_to_mat3`: Euler angles to a rotation matrix (double precision inside).
pub fn eul_o_to_mat3(e: [f32; 3], order: EulerOrder) -> Mat3 {
    from_m3(&eul_o_to_m3(e, order))
}

pub(crate) fn eul_o_to_m3(e: [f32; 3], order: EulerOrder) -> M3 {
    let ([i, j, k], parity) = order.info();
    let (ti, tj, th) = if parity {
        (f64::from(-e[i]), f64::from(-e[j]), f64::from(-e[k]))
    } else {
        (f64::from(e[i]), f64::from(e[j]), f64::from(e[k]))
    };
    let ci = libm::cos(ti);
    let cj = libm::cos(tj);
    let ch = libm::cos(th);
    let si = libm::sin(ti);
    let sj = libm::sin(tj);
    let sh = libm::sin(th);
    let cc = ci * ch;
    let cs = ci * sh;
    let sc = si * ch;
    let ss = si * sh;
    let mut m = [[0.0f32; 3]; 3];
    m[i][i] = (cj * ch) as f32;
    m[j][i] = (sj * sc - cs) as f32;
    m[k][i] = (sj * cc + ss) as f32;
    m[i][j] = (cj * sh) as f32;
    m[j][j] = (sj * ss + cc) as f32;
    m[k][j] = (sj * cs - sc) as f32;
    m[i][k] = (-sj) as f32;
    m[j][k] = (cj * si) as f32;
    m[k][k] = (cj * ci) as f32;
    m
}

/// `eulO_to_quat`: Euler angles to a w-first quaternion (double precision inside).
pub fn eul_o_to_quat(e: [f32; 3], order: EulerOrder) -> [f32; 4] {
    let ([i, j, k], parity) = order.info();
    let ti = f64::from(e[i]) * 0.5;
    let tj = f64::from(e[j]) * if parity { -0.5 } else { 0.5 };
    let th = f64::from(e[k]) * 0.5;
    let ci = libm::cos(ti);
    let cj = libm::cos(tj);
    let ch = libm::cos(th);
    let si = libm::sin(ti);
    let sj = libm::sin(tj);
    let sh = libm::sin(th);
    let cc = ci * ch;
    let cs = ci * sh;
    let sc = si * ch;
    let ss = si * sh;
    let mut a = [0.0f64; 3];
    a[i] = cj * sc - sj * cs;
    a[j] = cj * ss + sj * cc;
    a[k] = cj * cs - sj * sc;
    let mut q = [
        (cj * cc + sj * ss) as f32,
        a[0] as f32,
        a[1] as f32,
        a[2] as f32,
    ];
    if parity {
        q[j + 1] = -q[j + 1];
    }
    q
}

/// `mat3_normalized_to_eulo2`: the two Euler solutions of a normalized rotation matrix.
fn mat3_normalized_to_eul_o2(m: &M3, order: EulerOrder) -> ([f32; 3], [f32; 3]) {
    let ([i, j, k], parity) = order.info();
    let cy = libm::hypotf(m[i][i], m[i][j]);
    let mut e1 = [0.0f32; 3];
    let mut e2 = [0.0f32; 3];
    if cy > EULER_HYPOT_EPSILON {
        e1[i] = libm::atan2f(m[j][k], m[k][k]);
        e1[j] = libm::atan2f(-m[i][k], cy);
        e1[k] = libm::atan2f(m[i][j], m[i][i]);

        e2[i] = libm::atan2f(-m[j][k], -m[k][k]);
        e2[j] = libm::atan2f(-m[i][k], -cy);
        e2[k] = libm::atan2f(-m[i][j], -m[i][i]);
    } else {
        e1[i] = libm::atan2f(-m[k][j], m[j][j]);
        e1[j] = libm::atan2f(-m[i][k], cy);
        e1[k] = 0.0;
        e2 = e1;
    }
    if parity {
        for v in e1.iter_mut().chain(e2.iter_mut()) {
            *v = -*v;
        }
    }
    (e1, e2)
}

fn abs_sum(e: &[f32; 3]) -> f32 {
    e[0].abs() + e[1].abs() + e[2].abs()
}

fn abs_diff_sum(e: &[f32; 3], old: &[f32; 3]) -> f32 {
    (e[0] - old[0]).abs() + (e[1] - old[1]).abs() + (e[2] - old[2]).abs()
}

pub(crate) fn m3_normalized_to_eul_o(m: &M3, order: EulerOrder) -> [f32; 3] {
    let (e1, e2) = mat3_normalized_to_eul_o2(m, order);
    // Return the solution with the smallest values in it.
    if abs_sum(&e1) > abs_sum(&e2) { e2 } else { e1 }
}

pub(crate) fn m3_normalized_to_compatible_eul_o(
    m: &M3,
    old: [f32; 3],
    order: EulerOrder,
) -> [f32; 3] {
    let (e1, e2) = mat3_normalized_to_eul_o2(m, order);
    let e1 = compatible_eul(e1, old);
    let e2 = compatible_eul(e2, old);
    // Return the solution closest to `old`.
    if abs_diff_sum(&e1, &old) > abs_diff_sum(&e2, &old) {
        e2
    } else {
        e1
    }
}

/// `mat3_normalized_to_eulO`: matrix (columns already unit length) to Euler.
pub fn mat3_normalized_to_eul_o(m: &Mat3, order: EulerOrder) -> [f32; 3] {
    m3_normalized_to_eul_o(&to_m3(m), order)
}

/// `mat3_to_eulO`: normalizes the columns first.
pub fn mat3_to_eul_o(m: &Mat3, order: EulerOrder) -> [f32; 3] {
    m3_normalized_to_eul_o(&normalize_m3(&to_m3(m)), order)
}

/// `mat3_normalized_to_compatible_eulO`: the Euler solution closest to `old`.
pub fn mat3_normalized_to_compatible_eul_o(m: &Mat3, old: [f32; 3], order: EulerOrder) -> [f32; 3] {
    m3_normalized_to_compatible_eul_o(&to_m3(m), old, order)
}

/// `mat3_to_compatible_eulO`: normalizes the columns first.
pub fn mat3_to_compatible_eul_o(m: &Mat3, old: [f32; 3], order: EulerOrder) -> [f32; 3] {
    m3_normalized_to_compatible_eul_o(&normalize_m3(&to_m3(m)), old, order)
}

/// `quat_to_eulO` (w-first quaternion; expected to be normalized).
pub fn quat_to_eul_o(q: [f32; 4], order: EulerOrder) -> [f32; 3] {
    m3_normalized_to_eul_o(&to_m3(&quat_to_mat3(q)), order)
}

/// `quat_to_compatible_eulO`.
pub fn quat_to_compatible_eul_o(q: [f32; 4], old: [f32; 3], order: EulerOrder) -> [f32; 3] {
    m3_normalized_to_compatible_eul_o(&to_m3(&quat_to_mat3(q)), old, order)
}

/// `compatible_eul`: wraps each angle by 2π towards `old`, then flips single axes that moved
/// by more than π while the others stayed within π/2.
pub fn compatible_eul(eul: [f32; 3], old: [f32; 3]) -> [f32; 3] {
    let pi_thresh = std::f64::consts::PI as f32;
    let pi_x2 = 2.0f32 * std::f64::consts::PI as f32;
    let mut eul = eul;
    let mut deul = [0.0f32; 3];
    for i in 0..3 {
        deul[i] = eul[i] - old[i];
        if deul[i] > pi_thresh {
            eul[i] -= ((deul[i] / pi_x2) + 0.5).floor() * pi_x2;
            deul[i] = eul[i] - old[i];
        } else if deul[i] < -pi_thresh {
            eul[i] += ((-deul[i] / pi_x2) + 0.5).floor() * pi_x2;
            deul[i] = eul[i] - old[i];
        }
    }
    // `for (i = 0, j = 1, k = 2; i < 3; j = k, k = i++)`; comparisons in double, as in C.
    let pi = std::f64::consts::PI;
    let pi_2 = std::f64::consts::FRAC_PI_2;
    for (i, j, k) in [(0, 1, 2), (1, 2, 0), (2, 0, 1)] {
        if f64::from(deul[i].abs()) > pi
            && f64::from(deul[j].abs()) < pi_2
            && f64::from(deul[k].abs()) < pi_2
        {
            if deul[i] > 0.0 {
                eul[i] -= pi_x2;
            } else {
                eul[i] += pi_x2;
            }
        }
    }
    eul
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_round_trip() {
        for (n, o) in EulerOrder::ALL.iter().enumerate() {
            assert_eq!(EulerOrder::from_blender(o.as_str()), Some(*o));
            assert_eq!(o.rotmode(), n as i16 + 1);
        }
        assert_eq!(EulerOrder::from_blender("QUATERNION"), None);
    }

    #[test]
    fn xyz_is_rz_ry_rx() {
        let e = [0.3f32, -0.4, 1.1];
        let m = eul_o_to_mat3(e, EulerOrder::XYZ);
        let r =
            Mat3::from_rotation_z(e[2]) * Mat3::from_rotation_y(e[1]) * Mat3::from_rotation_x(e[0]);
        assert!(m.abs_diff_eq(r, 1e-6));
    }

    #[test]
    fn round_trips_every_order() {
        for o in EulerOrder::ALL {
            let e = [0.25f32, -0.5, 1.0];
            let m = eul_o_to_mat3(e, o);
            let back = mat3_to_eul_o(&m, o);
            for a in 0..3 {
                assert!((back[a] - e[a]).abs() < 1e-5, "{o:?} {back:?}");
            }
            let q = eul_o_to_quat(e, o);
            let mq = quat_to_mat3(q);
            assert!(mq.abs_diff_eq(m, 1e-6), "{o:?}");
        }
    }

    #[test]
    fn compatible_wraps_towards_old() {
        let e = compatible_eul([0.1, 0.0, 0.0], [6.3, 0.0, 0.0]);
        assert!((e[0] - (0.1 + 2.0 * std::f32::consts::PI)).abs() < 1e-5);
        let mut eu = Euler::new([0.0, 0.0, 3.0], EulerOrder::XYZ);
        eu.make_compatible([0.0, 0.0, -3.0]);
        assert!((eu.angles[2] - (3.0 - 2.0 * std::f32::consts::PI)).abs() < 1e-5);
    }
}
