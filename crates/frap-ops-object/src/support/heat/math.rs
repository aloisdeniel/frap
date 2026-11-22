// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BLI_math` helpers bone heat weighting reads, in `f32` and Blender's operation order
//! (no `mul_add`), so distances, cotangents and areas round as Blender's do.
//!
//! Ported from blender/source/blender/blenlib/intern/{math_geom.cc,math_vector.cc,
//! math_vector_inline.cc} (Blender 5.2.4, GPL-2.0-or-later).

pub(crate) type V3 = [f32; 3];

#[inline]
pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `madd_v3_v3v3fl`: `a + b * f`.
#[inline]
pub(crate) fn madd(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

/// `mul_v3_fl`
#[inline]
pub(crate) fn scale(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}

/// `dot_v3v3`
#[inline]
pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `cross_v3_v3v3`
#[inline]
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `len_v3`
#[inline]
pub(crate) fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// `len_v3v3(a, b)`: `|b - a|`.
#[inline]
pub(crate) fn len_v3v3(a: V3, b: V3) -> f32 {
    len(sub(b, a))
}

/// `normalize_v3`: the unit vector (zero when the squared length is `<= 1e-35`, also for a
/// NaN) and the original length.
#[inline]
pub(crate) fn normalize(a: V3) -> (V3, f32) {
    let d = dot(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        (scale(a, 1.0 / d), d)
    } else {
        ([0.0; 3], 0.0)
    }
}

/// `cross_tri_v3`
#[inline]
fn cross_tri(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ]
}

/// `normal_tri_v3`: the unit normal (the length is dropped).
#[inline]
pub(crate) fn normal_tri(v1: V3, v2: V3, v3: V3) -> V3 {
    normalize(cross_tri(v1, v2, v3)).0
}

/// `area_tri_v3`
#[inline]
pub(crate) fn area_tri(v1: V3, v2: V3, v3: V3) -> f32 {
    len(cross_tri(v1, v2, v3)) * 0.5
}

/// `cotangent_tri_weight_v3`: the cotangent of the angle at `v1`.
#[inline]
pub(crate) fn cotangent_tri_weight(v1: V3, v2: V3, v3: V3) -> f32 {
    let a = sub(v2, v1);
    let b = sub(v3, v1);
    let c_len = len(cross(a, b));
    if c_len > f32::EPSILON {
        dot(a, b) / c_len
    } else {
        0.0
    }
}

/// `saasin` / `safe_asinf`: `asinf` clamped to `[-1, 1]`.
#[inline]
fn safe_asin(a: f32) -> f32 {
    if a <= -1.0 {
        -std::f32::consts::FRAC_PI_2
    } else if a >= 1.0 {
        std::f32::consts::FRAC_PI_2
    } else {
        libm::asinf(a)
    }
}

/// `angle_normalized_v3v3`
#[inline]
fn angle_normalized(v1: V3, v2: V3) -> f32 {
    if dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asin(len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = [-v2[0], -v2[1], -v2[2]];
    std::f32::consts::PI - 2.0 * safe_asin(len_v3v3(v1, v2_n) / 2.0)
}

/// `angle_v3v3v3(a, b, c)`: the angle at `b`.
#[inline]
pub(crate) fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    let (v1, _) = normalize(sub(b, a));
    let (v2, _) = normalize(sub(b, c));
    angle_normalized(v1, v2)
}

/// `closest_to_line_segment_v3`: the point of the segment `l1 l2` nearest to `p`.
#[inline]
pub(crate) fn closest_to_line_segment(p: V3, l1: V3, l2: V3) -> V3 {
    // `closest_to_line_v3` → `closest_to_ray_v3`.
    let u = sub(l2, l1);
    let (lambda, cp) = if u == [0.0; 3] {
        (0.0, l1)
    } else {
        let h = sub(p, l1);
        let lambda = dot(u, h) / dot(u, u);
        (lambda, madd(l1, u, lambda))
    };
    // The flipped checks keep a NaN `lambda` (a zero-length segment) on `cp`.
    if lambda <= 0.0 {
        return l1;
    }
    if lambda >= 1.0 {
        return l2;
    }
    cp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cotangents_and_areas() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(cotangent_tri_weight(a, b, c), 0.0);
        assert!((cotangent_tri_weight(b, c, a) - 1.0).abs() < 1e-6);
        assert_eq!(area_tri(a, b, c), 0.5);
        assert_eq!(cotangent_tri_weight(a, a, c), 0.0);
        assert_eq!(normal_tri(a, b, c), [0.0, 0.0, 1.0]);
        let right = angle_v3v3v3(b, a, c);
        assert!((right - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((angle_v3v3v3(b, a, [-1.0, 0.0, 0.0]) - std::f32::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn segment_closest_points() {
        let (l1, l2) = ([0.0, 0.0, 0.0], [0.0, 0.0, 2.0]);
        assert_eq!(
            closest_to_line_segment([1.0, 0.0, 1.0], l1, l2),
            [0.0, 0.0, 1.0]
        );
        assert_eq!(closest_to_line_segment([1.0, 0.0, -1.0], l1, l2), l1);
        assert_eq!(closest_to_line_segment([1.0, 0.0, 5.0], l1, l2), l2);
        assert_eq!(closest_to_line_segment([1.0, 0.0, 5.0], l1, l1), l1);
    }
}
