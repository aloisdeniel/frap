// SPDX-License-Identifier: GPL-3.0-or-later
//! The world probe's octahedral parameterisation and its roughness ↔ mip mapping, on the CPU
//! (the WGSL twins are in `shaders/common.wgsl` and `shaders/bake.wgsl`; tests check one
//! against the other). Ported from blender/source/blender/draw/engines/eevee/shaders/
//! eevee_octahedron_lib.bsl.hh, eevee_lightprobe_sphere.bsl.hh and
//! eevee_lightprobe_sphere_bake.bsl.hh, and blender/source/blender/draw/engines/eevee/
//! eevee_defines.hh (`SPHERE_PROBE_*`).

/// `SPHERE_PROBE_MIPMAP_LEVELS`
pub const MIPMAP_LEVELS: u32 = 5;
/// `SPHERE_PROBE_MIP_MAX_ROUGHNESS`
pub const MIP_MAX_ROUGHNESS: f32 = 0.7;
/// `SPHERE_PROBE_MIX_START_ROUGHNESS`
pub const MIX_START_ROUGHNESS: f32 = 0.7;
/// `SPHERE_PROBE_MIX_END_ROUGHNESS`
pub const MIX_END_ROUGHNESS: f32 = 0.9;

/// `sqrt_fast` (blender/source/blender/gpu/shaders/common/gpu_shader_math_fast_lib.glsl).
pub fn sqrt_fast(v: f32) -> f32 {
    f32::from_bits((0x1fbd_1df5_i32.wrapping_add(v.to_bits() as i32 >> 1)) as u32)
}

/// `octahedral_uv_from_direction`
pub fn uv_from_direction(d: [f32; 3]) -> [f32; 2] {
    let s = d[0].abs() + d[1].abs() + d[2].abs();
    let mut co = [d[0] / s, d[1] / s, d[2] / s];
    if co[2] < 0.0 {
        let sign = |x: f32| if x >= 0.0 { 1.0 } else { -1.0 };
        let (x, y) = (co[0], co[1]);
        co[0] = (1.0 - y.abs()) * sign(x);
        co[1] = (1.0 - x.abs()) * sign(y);
    }
    [co[0] * 0.5 + 0.5, co[1] * 0.5 + 0.5]
}

/// `octahedral_uv_to_direction` (not normalized).
pub fn uv_to_direction(uv: [f32; 2]) -> [f32; 3] {
    let co = [uv[0] * 2.0 - 1.0, uv[1] * 2.0 - 1.0];
    let a = [co[0].abs(), co[1].abs()];
    let mut v = [co[0], co[1], 1.0 - (a[0] + a[1])];
    if a[0] + a[1] > 1.0 {
        // GLSL `sign` (0 at 0).
        let sign = |x: f32| {
            if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }
        };
        v[0] = (a[1] - 1.0) * -sign(co[0]);
        v[1] = (a[0] - 1.0) * -sign(co[1]);
    }
    v
}

/// `texel_to_direction(local_texel, extent)` (not normalized).
pub fn texel_to_direction(texel: [f32; 2], extent: u32) -> [f32; 3] {
    let e = (extent - 2) as f32;
    uv_to_direction([texel[0] / e, texel[1] / e])
}

/// `miplvl_scale_bias` for a probe filling its texture of mip-0 resolution `mip0_res`.
pub fn miplvl_scale_bias(mip: u32, mip0_res: u32, uv: [f32; 2]) -> [f32; 2] {
    let res = (mip0_res >> mip) as f32;
    let scale = (res - 2.0) / res;
    let offset = 0.5 / res;
    uv.map(|c| c * scale + offset)
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|c| c / l)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn triangle_solid_angle(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    2.0 * dot(a, cross(b, c))
        .abs()
        .atan2(1.0 + dot(b, c) + dot(a, c) + dot(a, b))
}

/// `octahedral_texel_solid_angle`: the solid angle a texel of the octahedral map stands for
/// (0 for the redundant last row and column).
pub fn texel_solid_angle(texel: [u32; 2], extent: u32) -> f32 {
    if texel[0] == extent - 1 || texel[1] == extent - 1 {
        return 0.0;
    }
    let half = extent >> 1;
    let padded = extent - 2;
    let wrap = |t: u32| if t >= half { padded - t } else { t };
    let w = [wrap(texel[0]), wrap(texel[1])];
    let hi = (half - 1) as f32;
    let corner = |dx: f32, dy: f32| {
        let c = [
            (w[0] as f32 + dx).clamp(0.0, hi),
            (w[1] as f32 + dy).clamp(0.0, hi),
        ];
        normalize(texel_to_direction(c, extent))
    };
    let (v00, v10, v01, v11) = (
        corner(-0.5, -0.5),
        corner(0.5, -0.5),
        corner(-0.5, 0.5),
        corner(0.5, 0.5),
    );
    let mut sa = triangle_solid_angle(v00, v10, v01) + triangle_solid_angle(v01, v10, v11);
    if w[0] == half - 1 {
        sa *= 2.0;
    }
    if w[1] == half - 1 {
        sa *= 2.0;
    }
    sa
}

/// `roughness_to_lod`: the probe mip (fractional) for a linear (UI) roughness.
pub fn roughness_to_lod(roughness: f32) -> f32 {
    let ratio = (roughness / MIP_MAX_ROUGHNESS).clamp(0.0, 1.0);
    let ratio_sqrt = sqrt_fast(ratio);
    let mip_ratio = ratio + (ratio_sqrt - ratio) * 0.4;
    mip_ratio * (MIPMAP_LEVELS - 1) as f32
}

/// `lod_to_roughness`: the linear roughness a probe mip is convolved for.
pub fn lod_to_roughness(lod: f32) -> f32 {
    let a = lod / (MIPMAP_LEVELS - 1) as f32;
    let (b, c) = (0.6f32, 0.4f32);
    let (b2, c2) = (b * b, c * c);
    let c4 = c2 * c2;
    let ratio = (-(4.0 * a * b * c2 + c4).sqrt() + 2.0 * a * b + c2) / (2.0 * b2);
    ratio * MIP_MAX_ROUGHNESS
}

/// `roughness_to_mix_fac`: how much of the SH (diffuse) world replaces the probe for very
/// rough reflections.
pub fn roughness_to_mix_fac(roughness: f32) -> f32 {
    let scale = 1.0 / (MIX_END_ROUGHNESS - MIX_START_ROUGHNESS);
    let bias = scale * MIX_START_ROUGHNESS;
    let x = (roughness * scale - bias).clamp(0.0, 1.0);
    x * x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_round_trips() {
        for &d in &[
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.3, -0.5, 0.2],
            [-0.7, 0.1, -0.6],
            [0.2, 0.9, -0.1],
        ] {
            let uv = uv_from_direction(d);
            let back = normalize(uv_to_direction(uv));
            let n = normalize(d);
            assert!(dot(back, n) > 0.99999, "{d:?} -> {uv:?} -> {back:?}");
        }
        // +Z maps to the centre, -Z to the corners.
        assert_eq!(uv_from_direction([0.0, 0.0, 1.0]), [0.5, 0.5]);
        assert_eq!(uv_to_direction([0.0, 0.0]), [0.0, 0.0, -1.0]);
    }

    #[test]
    fn texel_solid_angles_cover_the_sphere() {
        for extent in [16u32, 64, 256] {
            let mut sum = 0.0f64;
            for y in 0..extent {
                for x in 0..extent {
                    sum += texel_solid_angle([x, y], extent) as f64;
                }
            }
            let four_pi = 4.0 * std::f64::consts::PI;
            assert!((sum - four_pi).abs() < 1e-3 * four_pi, "{extent}: {sum}");
        }
    }

    #[test]
    fn lod_and_roughness() {
        assert!(
            roughness_to_lod(0.0).abs() < 1e-6,
            "sqrt_fast(0) is tiny, not 0"
        );
        assert!((roughness_to_lod(0.7) - 4.0).abs() < 0.02);
        assert_eq!(roughness_to_lod(1.0), roughness_to_lod(0.7));
        assert!(lod_to_roughness(0.0).abs() < 1e-7);
        assert!((lod_to_roughness(4.0) - 0.7).abs() < 1e-5);
        // `lod_to_roughness` inverts the exact-sqrt form of `roughness_to_lod`.
        for lod in [0.5f32, 1.0, 2.0, 3.0] {
            let r = lod_to_roughness(lod);
            let ratio = r / MIP_MAX_ROUGHNESS;
            let exact = (ratio + (ratio.sqrt() - ratio) * 0.4) * 4.0;
            assert!((exact - lod).abs() < 1e-4, "{lod}: {r} {exact}");
            // sqrt_fast is within a few percent.
            assert!((roughness_to_lod(r) - lod).abs() < 0.1);
        }
        assert_eq!(roughness_to_mix_fac(0.5), 0.0);
        assert_eq!(roughness_to_mix_fac(1.0), 1.0);
        assert!((roughness_to_mix_fac(0.8) - 0.25).abs() < 1e-5);
        assert!((sqrt_fast(4.0) - 2.0).abs() < 0.1);
    }
}
