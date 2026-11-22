// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BLI_math` helpers of the rip and loop tools (WP 8.3.3), with Blender's operation order.
//!
//! Ported from blender/source/blender/blenlib/intern/math_vector.cc, math_vector_inline.cc,
//! math_geom.cc, math_base_inline.cc and BLI_math_vector.hh (Blender 5.2.4,
//! GPL-2.0-or-later).

pub(crate) type V2 = [f32; 2];
pub(crate) type V3 = [f32; 3];

pub(crate) fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn add3(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn scale3(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}
pub(crate) fn dot3(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross3(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn len_squared3(a: V3) -> f32 {
    dot3(a, a)
}
pub(crate) fn len3(a: V3) -> f32 {
    len_squared3(a).sqrt()
}
pub(crate) fn len_v3v3(a: V3, b: V3) -> f32 {
    len3(sub3(a, b))
}
pub(crate) fn len_squared_v3v3(a: V3, b: V3) -> f32 {
    len_squared3(sub3(a, b))
}
pub(crate) fn mid_v3_v3v3(a: V3, b: V3) -> V3 {
    [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ]
}
/// `math::midpoint` of two `float3` (`(a + b) * 0.5`).
pub(crate) fn midpoint3(a: V3, b: V3) -> V3 {
    [
        (a[0] + b[0]) * 0.5,
        (a[1] + b[1]) * 0.5,
        (a[2] + b[2]) * 0.5,
    ]
}
/// `interp_v3_v3v3(r, a, b, t)`: `s = 1 - t; r = s * a + t * b`.
pub(crate) fn interp_v3_v3v3(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}
/// `math::interpolate(a, b, t)` for `float3`: `a * (1 - t) + b * t`.
pub(crate) fn interpolate3(a: V3, b: V3, t: f32) -> V3 {
    [
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
    ]
}

/// `normalize_v3(v)` (`normalize_v3_v3_length`: `v * (1 / |v|)`, zero below 1e-35 squared
/// length); returns the vector and its length.
pub(crate) fn normalize_v3(a: V3) -> (V3, f32) {
    let d = dot3(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let f = 1.0 / d;
        ([a[0] * f, a[1] * f, a[2] * f], d)
    } else {
        ([0.0; 3], 0.0)
    }
}

/// `math::normalize_and_get_length` for `float3` (`v / |v|`, zero below 1e-35 squared
/// length or with a NaN).
pub(crate) fn normalize_and_get_length(a: V3) -> (V3, f32) {
    let l2 = len_squared3(a);
    if l2 > 1.0e-35 {
        let l = l2.sqrt();
        ([a[0] / l, a[1] / l, a[2] / l], l)
    } else {
        ([0.0; 3], 0.0)
    }
}
/// `math::normalize` for `float3`.
pub(crate) fn normalize3(a: V3) -> V3 {
    normalize_and_get_length(a).0
}

/// `saasin` / `safe_asinf`.
pub(crate) fn safe_asinf(a: f32) -> f32 {
    if a <= -1.0 {
        -std::f32::consts::FRAC_PI_2
    } else if a >= 1.0 {
        std::f32::consts::FRAC_PI_2
    } else {
        libm::asinf(a)
    }
}

/// `angle_normalized_v3v3`.
pub(crate) fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if dot3(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = [-v2[0], -v2[1], -v2[2]];
    std::f32::consts::PI - 2.0 * safe_asinf(len_v3v3(v1, v2_n) / 2.0)
}

/// `closest_to_ray_v3`: the closest point to `p` on the ray (`ray_orig` for a zero
/// direction) and its factor.
pub(crate) fn closest_to_ray_v3(p: V3, ray_orig: V3, ray_dir: V3) -> (V3, f32) {
    if ray_dir == [0.0; 3] {
        return (ray_orig, 0.0);
    }
    let h = sub3(p, ray_orig);
    let lambda = dot3(ray_dir, h) / dot3(ray_dir, ray_dir);
    (
        [
            ray_orig[0] + ray_dir[0] * lambda,
            ray_orig[1] + ray_dir[1] * lambda,
            ray_orig[2] + ray_dir[2] * lambda,
        ],
        lambda,
    )
}

/// `closest_to_line_v3`.
pub(crate) fn closest_to_line_v3(p: V3, l1: V3, l2: V3) -> (V3, f32) {
    closest_to_ray_v3(p, l1, sub3(l2, l1))
}

// ------------------------------------------------------------------------------------ 2D

pub(crate) fn sub2(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}
pub(crate) fn dot2(a: V2, b: V2) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}
pub(crate) fn len_v2v2(a: V2, b: V2) -> f32 {
    let d = sub2(a, b);
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}
/// `interp_v2_v2v2(r, a, b, t)`.
pub(crate) fn interp_v2_v2v2(a: V2, b: V2, t: f32) -> V2 {
    let s = 1.0 - t;
    [s * a[0] + t * b[0], s * a[1] + t * b[1]]
}
/// `normalize_v2_length(v, unit_length)`.
pub(crate) fn normalize_v2_length(a: V2, unit_length: f32) -> V2 {
    let d = dot2(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let f = unit_length / d;
        [a[0] * f, a[1] * f]
    } else {
        [0.0; 2]
    }
}

/// `closest_to_line_segment_v2`.
pub(crate) fn closest_to_line_segment_v2(p: V2, l1: V2, l2: V2) -> V2 {
    // `closest_to_line_v2`.
    let u = sub2(l2, l1);
    let h = sub2(p, l1);
    let denom = dot2(u, u);
    let (cp, lambda) = if denom == 0.0 {
        (l1, 0.0)
    } else {
        let lambda = dot2(u, h) / denom;
        ([l1[0] + u[0] * lambda, l1[1] + u[1] * lambda], lambda)
    };
    // "flip checks for !finite case (when segment is a point)": a NaN factor keeps `cp`.
    if lambda <= 0.0 {
        l1
    } else if lambda >= 1.0 {
        l2
    } else {
        cp
    }
}

/// `dist_squared_to_line_segment_v2`.
pub(crate) fn dist_squared_to_line_segment_v2(p: V2, l1: V2, l2: V2) -> f32 {
    let c = closest_to_line_segment_v2(p, l1, l2);
    let d = sub2(c, p);
    d[0] * d[0] + d[1] * d[1]
}

/// `mul_m4_v4` on Blender's `m[col][row]` array.
pub(crate) fn mul_m4_v4(m: &[[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    let (x, y, z) = (v[0], v[1], v[2]);
    let mut r = [0.0f32; 4];
    for k in 0..4 {
        r[k] = x * m[0][k] + y * m[1][k] + z * m[2][k] + m[3][k] * v[3];
    }
    r
}

/// `math::transform_point(float4x4, float3)`: `view<3, 3>() * p + location()`.
pub(crate) fn transform_point(m: &[[f32; 4]; 4], p: V3) -> V3 {
    let mut r = [0.0f32; 3];
    for (k, rk) in r.iter_mut().enumerate() {
        *rk = ((0.0 + p[0] * m[0][k]) + p[1] * m[1][k]) + p[2] * m[2][k];
    }
    add3(r, [m[3][0], m[3][1], m[3][2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_of_zero_direction_is_a_third_of_pi() {
        // A zero first vector (a failed normalisation) gives pi/3 against any unit vector.
        let a = angle_normalized_v3v3([0.0; 3], [1.0, 0.0, 0.0]);
        assert!((a - std::f32::consts::FRAC_PI_3).abs() < 1e-6);
    }

    #[test]
    fn segment_distance_clamps_to_the_ends() {
        assert_eq!(
            dist_squared_to_line_segment_v2([2.0, 1.0], [0.0, 0.0], [1.0, 0.0]),
            2.0
        );
        assert_eq!(
            dist_squared_to_line_segment_v2([0.5, 1.0], [0.0, 0.0], [1.0, 0.0]),
            1.0
        );
        // A point segment.
        assert_eq!(
            dist_squared_to_line_segment_v2([0.0, 2.0], [0.0, 0.0], [0.0, 0.0]),
            4.0
        );
    }
}
