// SPDX-License-Identifier: GPL-3.0-or-later
// Shared IBL helpers (WP 5.2). Ported from blender/source/blender/draw/engines/eevee/shaders/
// eevee_octahedron_lib.bsl.hh, eevee_lightprobe_sphere.bsl.hh, eevee_spherical_harmonics.bsl.hh,
// eevee_sampling_lib.bsl.hh, blender/source/blender/gpu/shaders/common/
// gpu_shader_math_fast_lib.glsl and gpu_shader_math_matrix_construct_lib.glsl, and
// blender/source/blender/gpu/shaders/material/gpu_shader_material_tex_environment.glsl.
// Every consumer (the probe bake, the forward shader, the background) prepends this file.

const IBL_PI: f32 = 3.14159265358979;
const IBL_TAU: f32 = 6.28318530717959;
// SPHERE_PROBE_MIPMAP_LEVELS
const IBL_PROBE_MIPS: f32 = 5.0;
// SPHERE_PROBE_MIP_MAX_ROUGHNESS
const IBL_MIP_MAX_ROUGHNESS: f32 = 0.7;
// SPHERE_PROBE_MIX_START_ROUGHNESS / SPHERE_PROBE_MIX_END_ROUGHNESS
const IBL_MIX_START_ROUGHNESS: f32 = 0.7;
const IBL_MIX_END_ROUGHNESS: f32 = 0.9;

// `sqrt_fast` (gpu_shader_math_fast_lib.glsl): Blender's bit-trick square root.
fn ibl_sqrt_fast(v: f32) -> f32 {
    return bitcast<f32>(0x1fbd1df5i + (bitcast<i32>(v) >> 1u));
}

// `octahedral_uv_from_direction`
fn ibl_octahedral_uv_from_direction(dir: vec3<f32>) -> vec2<f32> {
    var co = dir / dot(vec3<f32>(1.0), abs(dir));
    if (co.z < 0.0) {
        let s = step(vec2<f32>(0.0), co.xy) * 2.0 - 1.0;
        co = vec3<f32>((1.0 - abs(co.yx)) * s, co.z);
    }
    return co.xy * 0.5 + 0.5;
}

// `octahedral_uv_to_direction` (not normalized)
fn ibl_octahedral_uv_to_direction(uv: vec2<f32>) -> vec3<f32> {
    let co = uv * 2.0 - 1.0;
    let a = abs(co);
    var v = vec3<f32>(co, 1.0 - (a.x + a.y));
    if (a.x + a.y > 1.0) {
        v = vec3<f32>((abs(co.yx) - 1.0) * -sign(co.xy), v.z);
    }
    return v;
}

// `texel_to_direction`: texel centres sit on the octahedron edges (`extent - 2`).
fn ibl_texel_to_direction(local_texel: vec2<f32>, extent: i32) -> vec3<f32> {
    return ibl_octahedral_uv_to_direction(local_texel / f32(extent - 2));
}

// `miplvl_scale_bias` for a probe that fills its whole texture (`uv_area.scale = 1`).
fn ibl_miplvl_scale_bias(mip_lvl: f32, mip0_res: i32, uv: vec2<f32>) -> vec2<f32> {
    let res = f32(mip0_res >> u32(mip_lvl));
    let scale = (res - 2.0) / res;
    let offset = 0.5 / res;
    return uv * scale + offset;
}

// `roughness_to_mix_fac`
fn ibl_roughness_to_mix_fac(roughness: f32) -> f32 {
    let scale = 1.0 / (IBL_MIX_END_ROUGHNESS - IBL_MIX_START_ROUGHNESS);
    let bias = scale * IBL_MIX_START_ROUGHNESS;
    let x = saturate(roughness * scale - bias);
    return x * x;
}

// `roughness_to_lod` (linear UI roughness → probe mip).
fn ibl_roughness_to_lod(roughness: f32) -> f32 {
    let ratio = saturate(roughness / IBL_MIP_MAX_ROUGHNESS);
    let ratio_sqrt = ibl_sqrt_fast(ratio);
    let mip_ratio = mix(ratio, ratio_sqrt, 0.4);
    return mip_ratio * (IBL_PROBE_MIPS - 1.0);
}

// `lod_to_roughness`
fn ibl_lod_to_roughness(lod: f32) -> f32 {
    let a = lod / (IBL_PROBE_MIPS - 1.0);
    let b = 0.6;
    let c = 0.4;
    let b2 = b * b;
    let c2 = c * c;
    let c4 = c2 * c2;
    let ratio = (-sqrt(4.0 * a * b * c2 + c4) + 2.0 * a * b + c2) / (2.0 * b2);
    return ratio * IBL_MIP_MAX_ROUGHNESS;
}

// `from_up_axis` (Duff et al. 2017), columns.
fn ibl_from_up_axis(up: vec3<f32>) -> mat3x3<f32> {
    let z_sign = select(-1.0, 1.0, up.z >= 0.0);
    let a = -1.0 / (z_sign + up.z);
    let b = up.x * up.y * a;
    return mat3x3<f32>(
        vec3<f32>(1.0 + z_sign * up.x * up.x * a, z_sign * b, -z_sign * up.x),
        vec3<f32>(b, z_sign + up.y * up.y * a, -up.y),
        up,
    );
}

// `hammersley_2d`
fn ibl_hammersley_2d(i: u32, count: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(count), f32(reverseBits(i)) * 2.3283064365386963e-10);
}

// `sample_circle`
fn ibl_sample_circle(rand: f32) -> vec2<f32> {
    let phi = (rand - 0.5) * IBL_TAU;
    let cos_phi = cos(phi);
    let sin_phi = sqrt(1.0 - cos_phi * cos_phi) * sign(phi);
    return vec2<f32>(cos_phi, sin_phi);
}

// `sample_uniform_cone`
fn ibl_sample_uniform_cone(rand: vec2<f32>, cos_angle: f32) -> vec3<f32> {
    let cos_theta = mix(cos_angle, 1.0, rand.x);
    let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));
    return vec3<f32>(sin_theta * ibl_sample_circle(rand.y), cos_theta);
}

// `colorspace::brightness_clamp_max`
fn ibl_brightness_clamp_max(color: vec3<f32>, limit: f32) -> vec3<f32> {
    let m = max(max(abs(color.r), abs(color.g)), abs(color.b));
    return color * saturate(limit / max(1e-8, m));
}

// Environment Texture, EQUIRECTANGULAR (`node_tex_environment_equirectangular`), after the
// Mapping node's Z rotation (`lookup = Rz(rotation) · direction`). Returns Blender's image
// coordinates (`v = 0` is the bottom row).
fn ibl_equirect_uv(direction: vec3<f32>, rotation: f32) -> vec2<f32> {
    let c = cos(rotation);
    let s = sin(rotation);
    let d = vec3<f32>(c * direction.x - s * direction.y, s * direction.x + c * direction.y,
                      direction.z);
    let l = length(d);
    if (l == 0.0) {
        return vec2<f32>(0.0);
    }
    let n = d / l;
    if (n.x == 0.0 || n.y == 0.0) {
        return vec2<f32>(0.0);
    }
    return vec2<f32>(-atan2(n.y, n.x) / IBL_TAU + 0.5,
                     atan2(n.z, length(n.xy)) / IBL_PI + 0.5);
}

// The texture coordinate of `ibl_equirect_uv` in a texture stored top row first.
fn ibl_equirect_tex_coord(direction: vec3<f32>, rotation: f32) -> vec2<f32> {
    let uv = ibl_equirect_uv(direction, rotation);
    return vec2<f32>(uv.x, 1.0 - uv.y);
}

// `SphericalHarmonicL1::evaluate_lambert`: the Lambertian radiance (irradiance / π) of the
// world for unit albedo, from the L1 coefficients `sh[0..4]` = L0.M0, L1.Mn1, L1.M0, L1.Mp1.
fn ibl_sh_lambert(sh0: vec3<f32>, sh1: vec3<f32>, sh2: vec3<f32>, sh3: vec3<f32>,
                  n: vec3<f32>) -> vec3<f32> {
    let l0 = 0.282094792 * sh0;
    let l1 = -0.488602512 * n.y * sh1 + 0.488602512 * n.z * sh2 - 0.488602512 * n.x * sh3;
    return max(vec3<f32>(0.0), l0 + l1 * (2.0 / 3.0));
}

// `bxdf_ggx_dominant_direction_reflection` (linear UI roughness).
fn ibl_dominant_direction_reflection(n: vec3<f32>, v: vec3<f32>, roughness: f32) -> vec3<f32> {
    let m = roughness * roughness;
    let r = -reflect(v, n);
    let smoothness = 1.0 - m;
    let fac = smoothness * (sqrt(smoothness) + m);
    return normalize(mix(n, r, fac));
}
