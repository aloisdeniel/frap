// SPDX-License-Identifier: GPL-3.0-or-later
//! Keyframes and their conversion to glTF sampler data.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/exp/animation/keyframes.py` (`Keyframe`: seconds `frame / (fps · fps_base)`, the
//! complete value of a target filled from its keyed `array_index`es),
//! `blender/com/gltf2_blender_math.py` (`list_to_mathutils`, `transform_location`,
//! `transform_rotation`, `transform_scale`, `swizzle_yup_*`, `mathutils_to_gltf`) and the
//! `__convert_keyframes` / `__gather_interpolation` / `fcurve_is_constant` functions of the
//! sampled and F-curve samplers.
//!
//! Values are kept as the add-on's raw lists (`Keyframe.__value`) together with the RNA
//! property they belong to (`location`, `rotation_euler`, `value`, …); `to_math` is
//! `list_to_mathutils` (an Euler list becomes a quaternion, with the XYZ order whatever the
//! object's rotation mode, and an axis-angle list reads its angle in **degrees**, both as the
//! add-on does). Vector and quaternion arithmetic is single precision, as `mathutils` stores
//! floats; shape-key weight lists stay Python doubles.

use frap_math::euler::{EulerOrder, eul_o_to_quat};
use frap_math::glam::{Mat3, Mat4, Vec3};
use frap_math::matrix::mat3_to_size;
use frap_math::quat::{QuatWxyz, axis_angle_to_quat, mat4_to_quat, normalize_qt, quat_to_mat3};

use crate::export::vtree::matmul;

/// glTF sampler interpolation (`get_gltf_interpolation`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Interp {
    /// `LINEAR`
    Linear,
    /// `STEP`
    Step,
    /// `CUBICSPLINE`
    Cubic,
}

impl Interp {
    /// `get_gltf_interpolation(interpolation)`: BEZIER → CUBICSPLINE, LINEAR → LINEAR,
    /// CONSTANT → STEP, anything else → the sampling fallback.
    pub(crate) fn from_blender(ipo: frap_anim::Interpolation, fallback: Interp) -> Interp {
        use frap_anim::Interpolation as I;
        match ipo {
            I::Bezier => Interp::Cubic,
            I::Linear => Interp::Linear,
            I::Constant => Interp::Step,
            _ => fallback,
        }
    }

    /// The glTF enum value.
    pub(crate) fn gltf(self) -> frap_gltf::json::animation::Interpolation {
        use frap_gltf::json::animation::Interpolation as G;
        match self {
            Interp::Linear => G::Linear,
            Interp::Step => G::Step,
            Interp::Cubic => G::CubicSpline,
        }
    }
}

/// One `Keyframe` of the add-on.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Key {
    /// `frame` (slid by `add_slide_data` when the animation slides).
    pub frame: f64,
    /// The complete value as a raw list (`Keyframe.__value`).
    pub value: Vec<f32>,
    /// The in-tangent control point (CUBICSPLINE only).
    pub in_tangent: Option<Vec<f32>>,
    /// The out-tangent control point (CUBICSPLINE only).
    pub out_tangent: Option<Vec<f32>>,
}

impl Key {
    /// A key with a value and no tangents.
    pub(crate) fn new(frame: f64, value: Vec<f32>) -> Key {
        Key {
            frame,
            value,
            in_tangent: None,
            out_tangent: None,
        }
    }
}

/// `Keyframe.get_target_len()` for a target property (`value`: the number of shape keys).
pub(crate) fn target_len(target: &str, morphs: usize) -> usize {
    match target {
        "delta_location"
        | "delta_rotation_euler"
        | "delta_scale"
        | "location"
        | "rotation_euler"
        | "scale" => 3,
        "delta_rotation_quaternion" | "rotation_axis_angle" | "rotation_quaternion" => 4,
        "value" => morphs,
        _ => 1,
    }
}

/// A value as `list_to_mathutils` returns it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Math {
    /// `Vector`
    Vec(Vec<f32>),
    /// `Quaternion` (w, x, y, z)
    Quat(QuatWxyz),
    /// A plain list (shape-key weights): Python doubles.
    List(Vec<f64>),
}

fn arr3(v: &[f32]) -> [f32; 3] {
    [
        v.first().copied().unwrap_or(0.0),
        v.get(1).copied().unwrap_or(0.0),
        v.get(2).copied().unwrap_or(0.0),
    ]
}

fn arr4(v: &[f32]) -> [f32; 4] {
    [
        v.first().copied().unwrap_or(0.0),
        v.get(1).copied().unwrap_or(0.0),
        v.get(2).copied().unwrap_or(0.0),
        v.get(3).copied().unwrap_or(0.0),
    ]
}

/// `list_to_mathutils(values, target)`.
pub(crate) fn to_math(values: &[f32], target: &str) -> Math {
    match target {
        "delta_rotation_euler" | "rotation_euler" => {
            // `Euler(values).to_quaternion()`: XYZ order.
            Math::Quat(eul_o_to_quat(arr3(values), EulerOrder::XYZ))
        }
        "rotation_axis_angle" => {
            // `Quaternion(axis, math.radians(angle))`: the add-on reads the angle as degrees.
            let angle = f64::from(values.first().copied().unwrap_or(0.0)).to_radians() as f32;
            let axis = arr3(values.get(1..).unwrap_or(&[]));
            Math::Quat(axis_angle_to_quat(axis, angle))
        }
        "rotation_quaternion" | "delta_rotation_quaternion" => Math::Quat(arr4(values)),
        "value" => Math::List(values.iter().map(|v| f64::from(*v)).collect()),
        _ => Math::Vec(values.to_vec()),
    }
}

/// The add-on's camera/light correction `Quaternion((√2/2, -√2/2, 0, 0))` as a matrix.
fn correction_m4() -> Mat4 {
    let h = (std::f64::consts::SQRT_2 / 2.0) as f32;
    Mat4::from_mat3(quat_to_mat3([h, -h, 0.0, 0.0]))
}

/// `gltf2_blender_math.transform(v, target, transform, need_rotation_correction)`.
pub(crate) fn transform(v: Math, target: &str, m: &Mat4, correction: bool) -> Math {
    match (v, target) {
        (Math::Vec(loc), "location" | "delta_location") => {
            // `(transform @ Matrix.Translation(loc) [@ correction]).to_translation()`.
            let t = Mat4::from_translation(Vec3::from(arr3(&loc)));
            let t = if correction {
                matmul(&t, &correction_m4())
            } else {
                t
            };
            let r = matmul(m, &t);
            Math::Vec(r.w_axis.truncate().to_array().to_vec())
        }
        (Math::Quat(q), _) => {
            // `rotation.normalize(); (transform @ rot.to_matrix().to_4x4() [@ corr])
            // .to_quaternion()`.
            let q = normalize_qt(q).0;
            let mut r = Mat4::from_mat3(quat_to_mat3(q));
            if correction {
                r = matmul(&r, &correction_m4());
            }
            Math::Quat(mat4_to_quat(&matmul(m, &r)))
        }
        (Math::Vec(s), "scale" | "delta_scale") => {
            // `(transform @ diag(scale)).to_scale()`.
            let d = Mat4::from_scale(Vec3::from(arr3(&s)));
            let r = matmul(m, &d);
            Math::Vec(mat3_to_size(&Mat3::from_mat4(r)).to_array().to_vec())
        }
        (v, _) => v,
    }
}

/// `swizzle_yup(v, target)`.
pub(crate) fn swizzle_yup(v: Math, target: &str) -> Math {
    match (v, target) {
        (Math::Vec(l), "location" | "delta_location") => {
            let l = arr3(&l);
            Math::Vec(vec![l[0], l[2], -l[1]])
        }
        (Math::Vec(s), "scale" | "delta_scale") => {
            let s = arr3(&s);
            Math::Vec(vec![s[0], s[2], s[1]])
        }
        (Math::Quat(q), _) => Math::Quat([q[0], q[1], q[3], -q[2]]),
        (v, _) => v,
    }
}

/// `mathutils_to_gltf(x)`: quaternions become (x, y, z, w).
pub(crate) fn to_gltf(v: &Math) -> Vec<f32> {
    match v {
        Math::Vec(v) => v.clone(),
        Math::Quat(q) => vec![q[1], q[2], q[3], q[0]],
        Math::List(l) => l.iter().map(|x| *x as f32).collect(),
    }
}

/// `fps * (tangent - value)` (single precision for vectors and quaternions, doubles for
/// lists).
fn tangent(t: &Math, v: &Math, fps: f64) -> Math {
    let f = fps as f32;
    match (t, v) {
        (Math::Vec(t), Math::Vec(v)) => Math::Vec(
            t.iter()
                .zip(v)
                .map(|(a, b)| f * (a - b))
                .collect::<Vec<f32>>(),
        ),
        (Math::Quat(t), Math::Quat(v)) => Math::Quat([
            f * (t[0] - v[0]),
            f * (t[1] - v[1]),
            f * (t[2] - v[2]),
            f * (t[3] - v[3]),
        ]),
        (Math::List(t), Math::List(v)) => {
            Math::List(t.iter().zip(v).map(|(a, b)| fps * (a - b)).collect())
        }
        (t, _) => t.clone(),
    }
}

/// How the keys of one channel become glTF values.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Convert<'a> {
    /// The RNA target property (`location`, `rotation_euler`, `value`, …).
    pub target: &'a str,
    /// The matrix the add-on transforms values with (`matrix_parent_inverse`, a bone's
    /// correction matrix, or the identity of the sampled object path).
    pub matrix: Mat4,
    /// Cameras and lights: the extra -90° X rotation.
    pub correction: bool,
    /// Swizzle to Y-up.
    pub yup: bool,
    /// `fps * fps_base` for tangents.
    pub fps: f64,
}

impl Convert<'_> {
    fn value(&self, raw: &[f32]) -> Math {
        let v = transform(
            to_math(raw, self.target),
            self.target,
            &self.matrix,
            self.correction,
        );
        if self.yup {
            swizzle_yup(v, self.target)
        } else {
            v
        }
    }

    /// The output data of `keys`: per key `[in-tangent] value [out-tangent]`, flattened.
    pub(crate) fn output(&self, keys: &[Key]) -> Vec<f32> {
        let mut out = Vec::new();
        for k in keys {
            let v = self.value(&k.value);
            let gv = to_gltf(&v);
            let mut cur: Vec<f32> = Vec::new();
            if let Some(t) = &k.in_tangent {
                cur.extend(to_gltf(&tangent(&self.value(t), &v, self.fps)));
            }
            cur.extend(gv);
            if let Some(t) = &k.out_tangent {
                cur.extend(to_gltf(&tangent(&self.value(t), &v, self.fps)));
            }
            out.extend(cur);
        }
        out
    }
}

/// `fcurve_is_constant`: every component's peak-to-peak range is below 1e-4.
pub(crate) fn is_constant(keys: &[Key]) -> bool {
    let Some(first) = keys.first() else {
        return true;
    };
    (0..first.value.len()).all(|i| {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for k in keys {
            let v = f64::from(k.value.get(i).copied().unwrap_or(0.0));
            lo = lo.min(v);
            hi = hi.max(v);
        }
        hi - lo < 0.0001
    })
}

/// `gather_*_sampled_keyframes`' optimisation: `keep` keeps a constant channel as its first
/// and last key; `None` drops it.
pub(crate) fn optimize(
    keys: Vec<Key>,
    optimize: bool,
    animated: bool,
    keep_unanimated: bool,
) -> Option<Vec<Key>> {
    if keys.is_empty() {
        return None;
    }
    let first_last = |mut k: Vec<Key>| {
        if k.len() >= 2 {
            let last = k.pop();
            k.truncate(1);
            k.extend(last);
        }
        k
    };
    if !optimize {
        if animated || keep_unanimated {
            return Some(keys);
        }
        return (!is_constant(&keys)).then_some(keys);
    }
    let cst = is_constant(&keys);
    if animated || keep_unanimated {
        Some(if cst { first_last(keys) } else { keys })
    } else {
        (!cst).then_some(keys)
    }
}

// Ported from blender/source/blender/python/mathutils/mathutils.cc (Blender 5.2.4,
// GPL-2.0-or-later): `EXPP_FloatsAreEqual`, `EXPP_VectorsAreEqual`.
/// `EXPP_FloatsAreEqual(af, bf, max_diff)`: the floats are at most `max_diff` ULPs apart.
fn floats_are_equal(af: f32, bf: f32, max_diff: i32) -> bool {
    let ai = af.to_bits() as i32;
    let bi = bf.to_bits() as i32;
    let test = -(((ai ^ bi) as u32 >> 31) as i32);
    let diff = (ai ^ (test & 0x7fff_ffff)).wrapping_sub(bi);
    let v1 = max_diff.wrapping_add(diff);
    let v2 = max_diff.wrapping_sub(diff);
    (v1 | v2) >= 0
}

/// `Vector.__eq__` / `Quaternion.__eq__` (`EXPP_VectorsAreEqual(a, b, size, 1)`; vectors of
/// different sizes are not equal).
pub(crate) fn mathutils_eq(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| floats_are_equal(*x, *y, 1))
}

/// `__gather_interpolation` of the sampled object and bone samplers.
pub(crate) fn sampled_interpolation(
    animated: bool,
    channel_interpolation: Interp,
    keys: &[Key],
    fallback: Interp,
) -> Interp {
    match keys.len() {
        n if n > 2 => {
            if channel_interpolation == Interp::Step {
                Interp::Step
            } else {
                fallback
            }
        }
        1 => {
            if !animated {
                Interp::Step
            } else if channel_interpolation == Interp::Cubic {
                fallback
            } else {
                channel_interpolation
            }
        }
        _ => {
            // `keyframes[0].value == keyframes[1].value` compares mathutils Vectors or
            // Quaternions: equal within one ULP per component.
            if !animated || mathutils_eq(&keys[0].value, &keys[1].value) {
                Interp::Step
            } else {
                fallback
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_list_becomes_xyz_quaternion() {
        let Math::Quat(q) = to_math(&[0.0, 0.0, 0.5], "rotation_euler") else {
            panic!("quaternion expected");
        };
        assert!((q[0] - 0.25f32.cos()).abs() < 1e-6);
        assert!((q[3] - 0.25f32.sin()).abs() < 1e-6);
    }

    #[test]
    fn axis_angle_reads_degrees() {
        let Math::Quat(q) = to_math(&[90.0, 0.0, 0.0, 1.0], "rotation_axis_angle") else {
            panic!("quaternion expected");
        };
        assert!((q[0] - (std::f32::consts::FRAC_PI_4).cos()).abs() < 1e-6);
    }

    #[test]
    fn yup_conversion_and_gltf_order() {
        let c = Convert {
            target: "rotation_quaternion",
            matrix: Mat4::IDENTITY,
            correction: false,
            yup: true,
            fps: 24.0,
        };
        // Blender (w, x, y, z) = rotation about Z → glTF (x, y, z, w) about Y.
        let h = (0.25f32).sin();
        let out = c.output(&[Key::new(1.0, vec![(0.25f32).cos(), 0.0, 0.0, h])]);
        assert!(out[0].abs() < 1e-7 && (out[1] - h).abs() < 1e-6 && out[2].abs() < 1e-7);
        let c = Convert {
            target: "location",
            ..c
        };
        assert_eq!(
            c.output(&[Key::new(1.0, vec![1.0, 2.0, 3.0])]),
            vec![1.0, 3.0, -2.0]
        );
        let c = Convert {
            target: "scale",
            ..c
        };
        assert_eq!(
            c.output(&[Key::new(1.0, vec![1.0, 2.0, 3.0])]),
            vec![1.0, 3.0, 2.0]
        );
    }

    #[test]
    fn tangents_are_relative_per_second() {
        let c = Convert {
            target: "location",
            matrix: Mat4::IDENTITY,
            correction: false,
            yup: false,
            fps: 24.0,
        };
        let mut k = Key::new(1.0, vec![0.0, 0.0, 0.0]);
        k.in_tangent = Some(vec![0.0, 0.0, 0.0]);
        k.out_tangent = Some(vec![0.5, 0.0, 0.0]);
        assert_eq!(
            c.output(&[k]),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 12.0, 0.0, 0.0]
        );
    }

    #[test]
    fn interpolation_rules() {
        let k = |v: f32| Key::new(0.0, vec![v]);
        let l = Interp::Linear;
        assert_eq!(
            sampled_interpolation(true, Interp::Cubic, &[k(0.), k(1.), k(2.)], l),
            l
        );
        assert_eq!(
            sampled_interpolation(true, Interp::Step, &[k(0.), k(1.), k(2.)], l),
            Interp::Step
        );
        assert_eq!(
            sampled_interpolation(true, l, &[k(1.), k(1.)], l),
            Interp::Step
        );
        assert_eq!(
            sampled_interpolation(false, l, &[k(0.), k(1.)], l),
            Interp::Step
        );
        assert_eq!(sampled_interpolation(true, l, &[k(0.), k(1.)], l), l);
        assert_eq!(sampled_interpolation(true, Interp::Cubic, &[k(0.)], l), l);
        assert_eq!(
            sampled_interpolation(false, Interp::Cubic, &[k(0.)], l),
            Interp::Step
        );
    }

    #[test]
    fn mathutils_equality_is_one_ulp() {
        let a = 0.090_628_445_f32;
        let b = f32::from_bits(a.to_bits() - 1);
        let c = f32::from_bits(a.to_bits() - 2);
        // Verified on 5.2.4: `Quaternion((1,0,a,0)) == Quaternion((1,0,b,0))` is True one ULP
        // apart, False two ULPs apart; `Vector((0,0,0)) == Vector((1e-7,0,0))` is False.
        assert!(mathutils_eq(&[1.0, 0.0, a, 0.0], &[1.0, 0.0, b, 0.0]));
        assert!(!mathutils_eq(&[1.0, 0.0, a, 0.0], &[1.0, 0.0, c, 0.0]));
        assert!(!mathutils_eq(&[0.0, 0.0, 0.0], &[1e-7, 0.0, 0.0]));
        assert!(!mathutils_eq(&[5.0, 0.0, 0.0], &[5.000_001, 0.0, 0.0]));
        assert!(mathutils_eq(&[0.0], &[-0.0]));
        assert!(!mathutils_eq(&[1.0, 2.0], &[1.0, 2.0, 3.0]));
        let l = Interp::Linear;
        let q = |z: f32| Key::new(0.0, vec![1.0, 0.0, z, 0.0]);
        assert_eq!(
            sampled_interpolation(true, l, &[q(a), q(b)], l),
            Interp::Step
        );
        assert_eq!(sampled_interpolation(true, l, &[q(a), q(c)], l), l);
    }

    #[test]
    fn optimisation_keeps_first_and_last_of_constant_channels() {
        let keys: Vec<Key> = (0..5).map(|i| Key::new(f64::from(i), vec![1.0])).collect();
        assert_eq!(optimize(keys.clone(), true, true, false).unwrap().len(), 2);
        assert!(optimize(keys.clone(), true, false, false).is_none());
        assert_eq!(optimize(keys.clone(), true, false, true).unwrap().len(), 2);
        assert_eq!(optimize(keys.clone(), false, true, false).unwrap().len(), 5);
        assert!(optimize(keys, false, false, false).is_none());
    }
}
