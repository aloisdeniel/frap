// SPDX-License-Identifier: GPL-3.0-or-later
// Split-sum material helpers over the LUTs of `WorldLighting` (WP 5.2; prepend common.wgsl).
// Ported from blender/source/blender/draw/engines/eevee/shaders/eevee_nodetree_lib.bsl.hh
// (`F_brdf_single_scatter`, `F_brdf_multi_scatter`, `brdf_f82_tint_lut`, `bsdf_lut`),
// eevee_bxdf_lut_lib.bsl.hh (LUT coordinates), eevee_utility_tx.bsl.hh (`sample_lut`,
// `sample_bsdf_lut`) and blender/source/blender/gpu/shaders/material/
// gpu_shader_material_principled.glsl (`node_bsdf_principled`, the layers without coat,
// sheen, subsurface, transmission and emission).

// UTIL_TEX_UV_SCALE / UTIL_TEX_UV_BIAS
const IBL_LUT_UV_SCALE: f32 = 63.0 / 64.0;
const IBL_LUT_UV_BIAS: f32 = 0.5 / 64.0;
// UTIL_BSDF_LAYER_COUNT
const IBL_BSDF_LAYERS: f32 = 16.0;

// `GGXBrdfData::sample_utility_tx`: (scale, bias, metal_bias).
fn ibl_brdf_lut(lut: texture_2d<f32>, s: sampler, cos_theta: f32, roughness: f32) -> vec3<f32> {
    let uv = vec2<f32>(roughness, sqrt(saturate(1.0 - cos_theta)));
    return textureSampleLevel(lut, s, uv * IBL_LUT_UV_SCALE + IBL_LUT_UV_BIAS, 0.0).rgb;
}

// `UtilityTexture::sample_bsdf_lut` (`uv` unscaled, `layer` in [0, 1]).
fn ibl_sample_bsdf_lut(bsdf: texture_2d_array<f32>, s: sampler, uv: vec2<f32>,
                       layer: f32) -> vec4<f32> {
    let suv = uv * IBL_LUT_UV_SCALE + IBL_LUT_UV_BIAS;
    let l = layer * IBL_BSDF_LAYERS;
    let lf = floor(l);
    let interp = l - lf;
    let lo = min(i32(lf), 15);
    let hi = min(lo + 1, 15);
    let a = textureSampleLevel(bsdf, s, suv, lo, 0.0);
    let b = textureSampleLevel(bsdf, s, suv, hi, 0.0);
    return mix(a, b, interp);
}

// `GGXBtdfGt1Data`: the transmission factor for IOR > 1 (`f0` is the amplitude
// `f0_from_ior`).
fn ibl_btdf_gt1(bsdf: texture_2d_array<f32>, s: sampler, cos_theta: f32, roughness: f32,
                f0: f32) -> f32 {
    let uv = vec2<f32>(sqrt(max(f0, 0.0)), sqrt(max(1.0 - cos_theta, 0.0)));
    return ibl_sample_bsdf_lut(bsdf, s, uv, saturate(roughness)).a;
}

// `GGXBsdfData` for IOR < 1: (scale, bias, transmission_factor).
fn ibl_bsdf_lt1(bsdf: texture_2d_array<f32>, s: sampler, cos_theta: f32, roughness: f32,
                ior: f32) -> vec3<f32> {
    let critical_cos = sqrt(1.0 - ior * ior);
    var y = cos_theta - critical_cos;
    y = y / select(critical_cos, 1.0 - critical_cos, y > 0.0);
    let coords = saturate(vec3<f32>(ior * ior, y * 0.5 + 0.5, roughness));
    return ibl_sample_bsdf_lut(bsdf, s, coords.xy, coords.z).rgb;
}

// `f0_from_ior`: the Fresnel amplitude at normal incidence.
fn ibl_f0_from_ior(eta: f32) -> f32 {
    return (eta - 1.0) / (eta + 1.0);
}

// `F_brdf_multi_scatter` with a BRDF LUT value.
fn ibl_f_brdf_multi_scatter(f0: vec3<f32>, f90: vec3<f32>, lut: vec3<f32>) -> vec3<f32> {
    let fss_ess = f0 * lut.x + f90 * lut.y;
    let ess = lut.x + lut.y;
    let ems = 1.0 - ess;
    let favg = f0 + (f90 - f0) / 21.0;
    return fss_ess / (1.0 - ems * favg);
}

// `brdf_f82_tint_lut` (multiscatter).
fn ibl_brdf_f82_tint(lut: texture_2d<f32>, s: sampler, f0: vec3<f32>, f82: vec3<f32>,
                     cos_theta: f32, roughness: f32) -> vec3<f32> {
    let l = ibl_brdf_lut(lut, s, cos_theta, roughness);
    var reflectance = ibl_f_brdf_multi_scatter(f0, vec3<f32>(1.0), l);
    let f = 6.0 / 7.0;
    let f5 = (f * f) * (f * f) * f;
    let f6 = (f * f) * (f * f) * (f * f);
    let f_schlick = mix(f0, vec3<f32>(1.0), f5);
    let b = f_schlick * (7.0 / f6) * (1.0 - f82);
    return reflectance - b * l.z;
}

// `bsdf_lut` reflectance (multiscatter, no transmission tint).
fn ibl_dielectric_reflectance(lut: texture_2d<f32>, bsdf: texture_2d_array<f32>, s: sampler,
                              f0_in: vec3<f32>, f90_in: vec3<f32>, cos_theta: f32,
                              roughness: f32, ior: f32) -> vec3<f32> {
    if (ior == 1.0) {
        return vec3<f32>(0.0);
    }
    let f0 = ibl_f0_from_ior(ior);
    var f90 = f90_in;
    var scale_bias_t: vec3<f32>;
    if (ior > 1.0) {
        if (all(f90 == vec3<f32>(1.0))) {
            f90 = vec3<f32>(saturate(2.33 / 0.33 * f0));
        }
        let b = ibl_brdf_lut(lut, s, cos_theta, roughness);
        scale_bias_t = vec3<f32>(b.x, b.y, ibl_btdf_gt1(bsdf, s, cos_theta, roughness, f0));
    } else {
        scale_bias_t = ibl_bsdf_lt1(bsdf, s, cos_theta, roughness, ior);
    }
    var reflectance = f0_in * scale_bias_t.x + f90 * scale_bias_t.y;
    let real_f0 = f0 * f0;
    let ess = real_f0 * scale_bias_t.x + scale_bias_t.y + (1.0 - real_f0) * scale_bias_t.z;
    let ems = 1.0 - ess;
    let favg = f0_in + (f90 - f0_in) / 21.0;
    return reflectance / (1.0 - ems * favg);
}

// The closure colours of a Principled BSDF with Base Color, Metallic, Roughness and IOR
// (Specular IOR Level 0.5, Specular Tint white, no coat, sheen, subsurface or transmission).
struct IblPrincipled {
    diffuse_color: vec3<f32>,
    reflection_color: vec3<f32>,
    roughness: f32,
}

fn ibl_principled(lut: texture_2d<f32>, bsdf: texture_2d_array<f32>, s: sampler,
                  base_color: vec3<f32>, metallic_in: f32, roughness_in: f32, ior_in: f32,
                  nv: f32) -> IblPrincipled {
    let metallic = saturate(metallic_in);
    let roughness = saturate(roughness_in);
    let ior = max(ior_in, 1e-5);
    let base = max(base_color, vec3<f32>(0.0));
    let clamped = min(base, vec3<f32>(1.0));
    var weight = 1.0;
    var reflection_color = vec3<f32>(0.0);
    if (metallic > 0.0) {
        let brdf = ibl_brdf_f82_tint(lut, s, clamped, vec3<f32>(1.0), nv, roughness);
        reflection_color = weight * metallic * brdf;
        weight *= max(1.0 - metallic, 0.0);
    }
    let f0 = ibl_f0_from_ior(ior);
    let f0_rgb = clamp(vec3<f32>(f0 * f0), vec3<f32>(0.0), vec3<f32>(1.0));
    let reflectance = ibl_dielectric_reflectance(lut, bsdf, s, f0_rgb, vec3<f32>(1.0), nv,
                                                 roughness, ior);
    reflection_color += weight * reflectance;
    weight *= max(1.0 - max(max(reflectance.r, reflectance.g), reflectance.b), 0.0);
    return IblPrincipled(weight * base, reflection_color, roughness);
}

// World lighting of the two closures: `diffuse_color * evaluate_lambert(N)` and
// `reflection_color * mix(probe(R, lod), evaluate_lambert(R), mix_fac)`
// (`LightprobeRenderData::eval`, world probe, normalization factor 1), from the
// `WorldLighting` fields as documented: `sh` in the world's frame with strength applied, the
// specular probe in the environment's frame at strength 1.
fn ibl_world_radiance(specular: texture_cube<f32>, s: sampler, sh: array<vec4<f32>, 9>,
                      rotation: f32, strength: f32, cl: IblPrincipled, n: vec3<f32>,
                      v: vec3<f32>) -> vec3<f32> {
    let diffuse = cl.diffuse_color * ibl_sh_lambert(sh, n);
    let r = ibl_dominant_direction_reflection(n, v, cl.roughness);
    let lod = ibl_roughness_to_lod(cl.roughness);
    let probe = textureSampleLevel(specular, s, ibl_world_lookup(r, rotation), lod).rgb *
                strength;
    let fac = ibl_roughness_to_mix_fac(cl.roughness);
    let spec = mix(probe, ibl_sh_lambert(sh, r), fac);
    return diffuse + cl.reflection_color * spec;
}
