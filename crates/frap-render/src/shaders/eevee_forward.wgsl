// SPDX-License-Identifier: GPL-3.0-or-later
// EEVEE-like forward uber-shader (WP 5.1, `eevee/pipeline.rs`). One shader for every
// material: features are uniform flags (`material.ints.x`), never pipeline permutations
// (§19.2). The shading-feature snippets of `eevee/features/*.rs` (WP 5.5) are appended to this
// source together with the generated dispatcher `eevee_features_apply`, which calls
// `feature_<name>(s, v)` for every feature whose flag bit is set.
//
// Ported from Blender 5.2.4 (GPL-2.0-or-later):
//   blender/source/blender/draw/engines/eevee/shaders/eevee_light_lib.bsl.hh
//   blender/source/blender/draw/engines/eevee/shaders/eevee_light_eval.bsl.hh
//   blender/source/blender/draw/engines/eevee/shaders/eevee_ltc_lib.bsl.hh
//   blender/source/blender/draw/engines/eevee/shaders/eevee_nodetree_lib.bsl.hh (bsdf_lut,
//     brdf_f82_tint_lut, F_brdf_*)
//   blender/source/blender/gpu/shaders/material/gpu_shader_material_principled.glsl
//   blender/source/blender/draw/engines/eevee/shaders/eevee_forward_lib.bsl.hh (combination,
//     brightness clamps)

const PI: f32 = 3.14159265358979;
const M_1_PI: f32 = 0.318309886183791;
const FLT_MAX: f32 = 3.402823e38;

// eevee_defines.hh
const UTIL_TEX_SIZE: f32 = 64.0;
const UTIL_BLUE_NOISE_LAYER: i32 = 0;
const UTIL_DISK_INTEGRAL_LAYER: i32 = 1;
const UTIL_LTC_MAT_LAYER: i32 = 2;
const UTIL_BRDF_LAYER: i32 = 3;
const UTIL_BSDF_LAYER: i32 = 4;
const UTIL_BSDF_LAYER_COUNT: f32 = 16.0;

// eLightType
const LIGHT_SUN: u32 = 0u;
const LIGHT_OMNI_SPHERE: u32 = 10u;
const LIGHT_OMNI_DISK: u32 = 11u;
const LIGHT_SPOT_SPHERE: u32 = 12u;
const LIGHT_SPOT_DISK: u32 = 13u;
const LIGHT_RECT: u32 = 20u;
const LIGHT_ELLIPSE: u32 = 21u;

// Shadow kinds (`lights::ShadowKind`).
const SHADOW_NONE: u32 = 0u;
const SHADOW_CUBE: u32 = 1u;
const SHADOW_SPOT: u32 = 2u;
const SHADOW_CASCADES: u32 = 3u;

// `material.ints.y`: blend mode (`pipeline::BlendMode`).
const BLEND_OPAQUE: u32 = 0u;
const BLEND_CLIP: u32 = 1u;
const BLEND_DITHERED: u32 = 2u;
const BLEND_BLENDED: u32 = 3u;

// `material.ints.w` bits.
const MAT_DOUBLE_SIDED: u32 = 1u;
const MAT_UNLIT: u32 = 2u;
const MAT_MULTISCATTER: u32 = 4u;
// Refraction traces the opaque pass (`use_raytrace_refraction` with scene ray tracing, WP 5.5);
// otherwise it samples the world probe.
const MAT_SCREEN_REFRACTION: u32 = 8u;

// `frame.counts.w` bits.
const FRAME_FILM_TRANSPARENT: u32 = 1u;
const FRAME_WORLD_HDRI: u32 = 2u;
const FRAME_PERSPECTIVE: u32 = 4u;
const FRAME_REFRACTION: u32 = 8u;

const LIGHT_ATTENUATION_THRESHOLD: f32 = 1e-6;

struct Frame {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // xyz camera position.
    camera_pos: vec4<f32>,
    // xyz world view direction (orthographic cameras).
    camera_fwd: vec4<f32>,
    // rgb: constant world radiance (color × strength).
    world_color: vec4<f32>,
    // x, y: render size; z, w: 1 / size.
    resolution: vec4<f32>,
    // x: light count, y: sample index, z: specular mips (HDRI), w: FRAME_* bits.
    counts: vec4<u32>,
    // x: clamp_surface_direct, y: clamp_surface_indirect (0 = off), z: transparency random,
    // w: unused.
    clamp: vec4<f32>,
    // HDRI world (WP 5.2): x cos(rotation), y sin(rotation), z strength.
    world_params: vec4<f32>,
    // xy: PCF rotation random, z: shadow maps per row of an atlas layer, w: 1 / map size.
    shadow: vec4<f32>,
    // HDRI world irradiance (L2 SH, rgb).
    sh: array<vec4<f32>, 9>,
};

struct Light {
    x_axis: vec4<f32>,
    y_axis: vec4<f32>,
    z_axis: vec4<f32>,
    position: vec4<f32>,
    color: vec4<f32>,
    // diffuse, specular, transmission, volume.
    power: vec4<f32>,
    // influence_radius_invsqr_surface, influence_radius_max, shape_radius, shadow_radius.
    local: vec4<f32>,
    // spot_size_inv.xy, spot_mul, spot_bias.
    spot: vec4<f32>,
    // area half size xy, sun shape radius, sun shadow angle.
    shape: vec4<f32>,
    // xyz shadow origin (sun: direction to the jittered sun), w texel scale.
    shadow_origin: vec4<f32>,
    cascade_far: vec4<f32>,
    cascade_texel: vec4<f32>,
    // x: shadow_filter_radius.
    shadow_params: vec4<f32>,
    // type, shadow kind, first shadow map (eevee/shadows.rs Plan), unused.
    ints: vec4<u32>,
};

struct Draw {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    // x: 1 when the object matrix flips the winding, y: object index.
    flags: vec4<u32>,
};

// One texture slot of a material: uv' = (m0.xy·uv + m0.z, m1.xy·uv + m1.z) in glTF
// convention (v down); m0.w: UV channel (0, 1), m1.w: 1 when the slot is bound.
struct TexSlot {
    m0: vec4<f32>,
    m1: vec4<f32>,
};

// `pipeline::MaterialParams` as the GPU sees it (`pipeline::GpuMaterial`).
struct Material {
    base_color: vec4<f32>,
    // metallic, roughness, ior, specular_ior_level.
    pbr: vec4<f32>,
    // rgb specular tint, w alpha cutoff.
    specular_tint: vec4<f32>,
    // rgb emission colour, w emission strength.
    emission: vec4<f32>,
    // coat weight, roughness, ior, normal-map scale.
    coat: vec4<f32>,
    // rgb coat tint, w normal-map scale.
    coat_tint: vec4<f32>,
    // rgb sheen tint, w sheen roughness.
    sheen: vec4<f32>,
    // transmission weight, sheen weight, occlusion strength, unused.
    misc: vec4<f32>,
    // Values of the shading features (`MaterialParams::extra`).
    extra: array<vec4<f32>, 4>,
    // Texture slot of each channel (-1: none):
    // [0] base colour, metallic-roughness (B, G), normal, emission;
    // [1] coat weight (R), coat roughness (G), coat normal, sheen colour (RGB);
    // [2] sheen roughness (A), transmission (R), specular level (A), specular tint (RGB);
    // [3] feature-defined.
    tex: array<vec4<i32>, 4>,
    // x: feature flag bits, y: blend mode, z: vertex-colour mode, w: MAT_* bits.
    ints: vec4<u32>,
    slots: array<TexSlot, 8>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read> shadow_mats: array<mat4x4<f32>>;
@group(0) @binding(3) var util_tx: texture_2d_array<f32>;
@group(0) @binding(4) var linear_samp: sampler;
@group(0) @binding(5) var shadow_tx: texture_depth_2d_array;
@group(0) @binding(6) var shadow_samp: sampler_comparison;
@group(0) @binding(7) var world_spec_tx: texture_cube<f32>;
@group(0) @binding(8) var world_bg_tx: texture_2d<f32>;
@group(0) @binding(9) var refraction_tx: texture_2d<f32>;

@group(1) @binding(0) var<uniform> draw: Draw;

@group(2) @binding(0) var<uniform> material: Material;
@group(2) @binding(1) var tex0: texture_2d<f32>;
@group(2) @binding(2) var tex1: texture_2d<f32>;
@group(2) @binding(3) var tex2: texture_2d<f32>;
@group(2) @binding(4) var tex3: texture_2d<f32>;
@group(2) @binding(5) var tex4: texture_2d<f32>;
@group(2) @binding(6) var tex5: texture_2d<f32>;
@group(2) @binding(7) var tex6: texture_2d<f32>;
@group(2) @binding(8) var tex7: texture_2d<f32>;
@group(2) @binding(9) var samp0: sampler;
@group(2) @binding(10) var samp1: sampler;
@group(2) @binding(11) var samp2: sampler;
@group(2) @binding(12) var samp3: sampler;
@group(2) @binding(13) var samp4: sampler;
@group(2) @binding(14) var samp5: sampler;
@group(2) @binding(15) var samp6: sampler;
@group(2) @binding(16) var samp7: sampler;

// ---------------------------------------------------------------------------------------
// Vertex stage

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) tangent: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_nor: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) tangent: vec4<f32>,
};

@vertex
fn vs_forward(v: VertexIn) -> VertexOut {
    var o: VertexOut;
    let wp = draw.model * vec4<f32>(v.position, 1.0);
    o.clip = frame.view_proj * wp;
    o.world_pos = wp.xyz;
    o.world_nor = (draw.normal_matrix * vec4<f32>(v.normal, 0.0)).xyz;
    o.uv0 = v.uv0;
    o.uv1 = v.uv1;
    o.color = v.color;
    let t = (draw.model * vec4<f32>(v.tangent.xyz, 0.0)).xyz;
    // A mirrored object flips the bitangent.
    let sign = select(v.tangent.w, -v.tangent.w, draw.flags.x != 0u);
    o.tangent = vec4<f32>(t, sign);
    return o;
}

// ---------------------------------------------------------------------------------------
// Utilities

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn sqr(x: f32) -> f32 {
    return x * x;
}

fn safe_normalize(v: vec3<f32>) -> vec3<f32> {
    let l = length(v);
    return select(vec3<f32>(1.0, 0.0, 0.0), v / l, l > 1e-35);
}

fn reduce_max(v: vec3<f32>) -> f32 {
    return max(v.x, max(v.y, v.z));
}

// Sample at uv with scale and bias so that the uv bounds lie on texel centres.
fn sample_lut(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    let c = uv * ((UTIL_TEX_SIZE - 1.0) / UTIL_TEX_SIZE) + 0.5 / UTIL_TEX_SIZE;
    return textureSampleLevel(util_tx, linear_samp, c, layer, 0.0);
}

fn sample_bsdf_lut(uv: vec2<f32>, layer_coord: f32) -> vec4<f32> {
    let c = uv * ((UTIL_TEX_SIZE - 1.0) / UTIL_TEX_SIZE) + 0.5 / UTIL_TEX_SIZE;
    let layer = layer_coord * UTIL_BSDF_LAYER_COUNT + f32(UTIL_BSDF_LAYER);
    let lf = floor(layer);
    let interp = layer - lf;
    let lo = textureSampleLevel(util_tx, linear_samp, c, i32(lf), 0.0);
    let hi = textureSampleLevel(util_tx, linear_samp, c, min(i32(lf) + 1, UTIL_BSDF_LAYER + 15), 0.0);
    return mix(lo, hi, interp);
}

// Material textures: slot `i` sampled with explicit gradients (no uniformity requirement).
struct Inputs {
    P: vec3<f32>,
    Ng: vec3<f32>,
    N: vec3<f32>,
    V: vec3<f32>,
    uv0: vec2<f32>,
    uv1: vec2<f32>,
    duv0dx: vec2<f32>,
    duv0dy: vec2<f32>,
    duv1dx: vec2<f32>,
    duv1dy: vec2<f32>,
    color: vec4<f32>,
    // World tangent (xyz) and bitangent sign (w).
    tangent: vec4<f32>,
    frag: vec4<f32>,
    front: bool,
};

fn tex_raw(i: i32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    switch i {
        case 0: { return textureSampleGrad(tex0, samp0, uv, dx, dy); }
        case 1: { return textureSampleGrad(tex1, samp1, uv, dx, dy); }
        case 2: { return textureSampleGrad(tex2, samp2, uv, dx, dy); }
        case 3: { return textureSampleGrad(tex3, samp3, uv, dx, dy); }
        case 4: { return textureSampleGrad(tex4, samp4, uv, dx, dy); }
        case 5: { return textureSampleGrad(tex5, samp5, uv, dx, dy); }
        case 6: { return textureSampleGrad(tex6, samp6, uv, dx, dy); }
        case 7: { return textureSampleGrad(tex7, samp7, uv, dx, dy); }
        default: { return vec4<f32>(1.0); }
    }
}

// The texture of slot `i` at the fragment (white when the slot is -1 or unbound). Blender UVs
// (v up) go through the slot's glTF-convention transform, then sample the image top-down.
fn tex_sample(i: i32, v: ptr<function, Inputs>) -> vec4<f32> {
    if (i < 0 || i > 7) {
        return vec4<f32>(1.0);
    }
    let s = material.slots[i];
    if (s.m1.w == 0.0) {
        return vec4<f32>(1.0);
    }
    let ch1 = s.m0.w > 0.5;
    var uv = select((*v).uv0, (*v).uv1, ch1);
    var dx = select((*v).duv0dx, (*v).duv1dx, ch1);
    var dy = select((*v).duv0dy, (*v).duv1dy, ch1);
    uv = vec2<f32>(uv.x, 1.0 - uv.y);
    dx = vec2<f32>(dx.x, -dx.y);
    dy = vec2<f32>(dy.x, -dy.y);
    let t = vec2<f32>(dot(s.m0.xy, uv) + s.m0.z, dot(s.m1.xy, uv) + s.m1.z);
    let tdx = vec2<f32>(dot(s.m0.xy, dx), dot(s.m1.xy, dx));
    let tdy = vec2<f32>(dot(s.m0.xy, dy), dot(s.m1.xy, dy));
    return tex_raw(i, t, tdx, tdy);
}

// ---------------------------------------------------------------------------------------
// Surface: the Principled BSDF inputs, filled from the material, then by the features.

struct Surface {
    base_color: vec3<f32>,
    alpha: f32,
    metallic: f32,
    roughness: f32,
    ior: f32,
    specular_ior_level: f32,
    specular_tint: vec3<f32>,
    N: vec3<f32>,
    emission: vec3<f32>,
    emission_strength: f32,
    coat_weight: f32,
    coat_roughness: f32,
    coat_ior: f32,
    coat_tint: vec3<f32>,
    coat_N: vec3<f32>,
    sheen_weight: f32,
    sheen_roughness: f32,
    sheen_tint: vec3<f32>,
    transmission_weight: f32,
};

fn surface_init(v: ptr<function, Inputs>) -> Surface {
    var s: Surface;
    let m = material;
    var base = m.base_color;
    let tb = m.tex[0];
    if (tb.x >= 0) {
        base *= tex_sample(tb.x, v);
    }
    s.base_color = base.rgb;
    s.alpha = base.a;
    s.metallic = m.pbr.x;
    s.roughness = m.pbr.y;
    if (tb.y >= 0) {
        let mr = tex_sample(tb.y, v);
        s.metallic *= mr.b;
        s.roughness *= mr.g;
    }
    s.ior = m.pbr.z;
    s.specular_ior_level = m.pbr.w;
    s.specular_tint = m.specular_tint.rgb;
    let t2 = m.tex[2];
    if (t2.z >= 0) {
        s.specular_ior_level *= tex_sample(t2.z, v).a;
    }
    if (t2.w >= 0) {
        s.specular_tint *= tex_sample(t2.w, v).rgb;
    }
    s.N = (*v).N;
    s.emission = vec3<f32>(0.0);
    s.emission_strength = 0.0;
    s.coat_weight = 0.0;
    s.coat_roughness = 0.03;
    s.coat_ior = 1.5;
    s.coat_tint = vec3<f32>(1.0);
    s.coat_N = (*v).N;
    s.sheen_weight = 0.0;
    s.sheen_roughness = 0.5;
    s.sheen_tint = vec3<f32>(1.0);
    s.transmission_weight = 0.0;
    return s;
}

// ---------------------------------------------------------------------------------------
// BSDF LUTs (eevee_nodetree_lib)

fn f0_from_ior(eta: f32) -> f32 {
    return (eta - 1.0) / (eta + 1.0);
}

fn F0_from_ior(eta: f32) -> f32 {
    return sqr(f0_from_ior(eta));
}

fn ior_from_F0(F0: f32) -> f32 {
    let f = sqrt(clamp(F0, 0.0, 0.99));
    return (-f - 1.0) / (f - 1.0);
}

// GGXBrdfData: (scale, bias, metal_bias).
fn brdf_lut(cos_theta: f32, roughness: f32) -> vec3<f32> {
    return sample_lut(vec2<f32>(roughness, sqrt(saturate(1.0 - cos_theta))), UTIL_BRDF_LAYER).xyz;
}

struct BsdfOut {
    reflectance: vec3<f32>,
    transmittance: vec3<f32>,
};

fn bsdf_lut(F0_in: vec3<f32>, F90_in: vec3<f32>, transmission_tint: vec3<f32>, cos_theta: f32,
            roughness: f32, ior: f32, do_multiscatter: bool) -> BsdfOut {
    var o: BsdfOut;
    if (ior == 1.0) {
        o.reflectance = vec3<f32>(0.0);
        o.transmittance = transmission_tint;
        return o;
    }
    var F90 = F90_in;
    let F0 = F0_in;
    let f0 = f0_from_ior(ior);
    var scale: f32;
    var bias: f32;
    var transmission_factor: f32;
    if (ior > 1.0) {
        if (all(F90 == vec3<f32>(1.0))) {
            F90 = vec3<f32>(saturate(2.33 / 0.33 * f0));
        }
        let b = brdf_lut(cos_theta, roughness);
        scale = b.x;
        bias = b.y;
        let c = vec3<f32>(sqrt(f0), sqrt(1.0 - cos_theta), roughness);
        transmission_factor = sample_bsdf_lut(c.xy, c.z).w;
    } else {
        // GGXBsdfData::lut_coords_get: IOR is the sine of the critical angle.
        let critical_cos = sqrt(1.0 - ior * ior);
        var c: vec3<f32>;
        c.x = ior * ior;
        c.y = cos_theta - critical_cos;
        c.y = c.y / select(critical_cos, 1.0 - critical_cos, c.y > 0.0);
        c.y = c.y * 0.5 + 0.5;
        c.z = roughness;
        c = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
        let d = sample_bsdf_lut(c.xy, c.z);
        scale = d.x;
        bias = d.y;
        transmission_factor = d.z;
    }
    o.reflectance = F0 * scale + F90 * bias;
    o.transmittance = (vec3<f32>(1.0) - F0) * transmission_factor * transmission_tint;
    if (do_multiscatter) {
        let real_F0 = f0 * f0;
        let Ess = real_F0 * scale + bias + (1.0 - real_F0) * transmission_factor;
        let Ems = 1.0 - Ess;
        let Favg = select(transmission_tint, F0 + (F90 - F0) / 21.0,
                          all(transmission_tint == vec3<f32>(0.0)));
        let k = 1.0 / (vec3<f32>(1.0) - Ems * Favg);
        o.reflectance *= k;
        o.transmittance *= k;
    }
    return o;
}

fn brdf_f82_tint_lut(F0: vec3<f32>, F82: vec3<f32>, cos_theta: f32, roughness: f32,
                     do_multiscatter: bool) -> vec3<f32> {
    let lut = brdf_lut(cos_theta, roughness);
    var reflectance = F0 * lut.x + lut.y;
    if (do_multiscatter) {
        let Ess = lut.x + lut.y;
        let Ems = 1.0 - Ess;
        let Favg = F0 + (vec3<f32>(1.0) - F0) / 21.0;
        reflectance = reflectance / (vec3<f32>(1.0) - Ems * Favg);
    }
    let f = 6.0 / 7.0;
    let f5 = (f * f) * (f * f) * f;
    let f6 = (f * f) * (f * f) * (f * f);
    let F_schlick = mix(F0, vec3<f32>(1.0), f5);
    let b = F_schlick * (7.0 / f6) * (vec3<f32>(1.0) - F82);
    return reflectance - b * lut.z;
}

fn principled_sheen(NV: f32, rough: f32) -> f32 {
    let den = 35.6694 * rough * rough - 24.4269 * rough * NV - 0.1405 * NV * NV +
              6.1211 * rough + 0.28105 * NV - 0.1405;
    let num = 58.5299 * rough * rough - 85.0941 * rough * NV + 9.8955 * NV * NV +
              1.9250 * rough + 74.2268 * NV - 0.2246;
    return saturate(den / num);
}

fn slab_transmittance_at_angle(color: vec3<f32>, cos_theta_i: f32, ior: f32) -> vec3<f32> {
    let inv_cos_theta_t = ior / sqrt(max(sqr(ior) - (1.0 - sqr(cos_theta_i)), 1e-8));
    return pow(color, vec3<f32>(inv_cos_theta_t));
}

// The closures of `node_bsdf_principled` (EEVEE): diffuse, one reflection, the coat
// reflection, the refraction and the emission, colours with the layer weights applied.
struct Closures {
    diffuse_color: vec3<f32>,
    diffuse_N: vec3<f32>,
    reflection_color: vec3<f32>,
    reflection_N: vec3<f32>,
    reflection_roughness: f32,
    coat_color: vec3<f32>,
    coat_N: vec3<f32>,
    coat_roughness: f32,
    refraction_color: vec3<f32>,
    refraction_roughness: f32,
    emission: vec3<f32>,
};

fn principled(s_in: Surface, V: vec3<f32>, do_multiscatter: bool) -> Closures {
    let s = s_in;
    var c: Closures;
    let metallic = saturate(s.metallic);
    let roughness = saturate(s.roughness);
    let ior = max(s.ior, 1e-5);
    let specular_ior_level = max(s.specular_ior_level, 0.0);
    let specular_tint = max(s.specular_tint, vec3<f32>(0.0));
    let transmission_weight = saturate(s.transmission_weight);
    let coat_weight = max(s.coat_weight, 0.0);
    let coat_roughness = saturate(s.coat_roughness);
    let coat_ior = max(s.coat_ior, 1.0);
    var coat_tint = max(s.coat_tint, vec3<f32>(0.0));
    let sheen_weight = max(s.sheen_weight, 0.0);
    let sheen_roughness = saturate(s.sheen_roughness);
    let sheen_tint = max(s.sheen_tint, vec3<f32>(0.0));
    let base_color = max(s.base_color, vec3<f32>(0.0));
    let clamped_base_color = min(base_color, vec3<f32>(1.0));
    let N = s.N;
    let CN = s.coat_N;
    let NV = dot(N, V);
    var weight = 1.0;

    // Sheen
    var sheen_data_color = vec3<f32>(0.0);
    if (sheen_weight > 0.0) {
        var sheen_NV = NV;
        if (coat_weight > 0.0) {
            let sheen_N = safe_normalize(mix(N, CN, saturate(coat_weight)));
            sheen_NV = dot(sheen_N, V);
        }
        sheen_NV = saturate(sheen_NV);
        let sheen_color = sheen_weight * sheen_tint * principled_sheen(sheen_NV, sheen_roughness);
        sheen_data_color = weight * sheen_color;
        weight *= max(1.0 - reduce_max(sheen_color), 0.0);
    }

    // Coat
    c.coat_color = vec3<f32>(0.0);
    c.coat_N = CN;
    c.coat_roughness = coat_roughness;
    if (coat_weight > 0.0) {
        let coat_NV = dot(CN, V);
        let F0c = F0_from_ior(coat_ior);
        let r = bsdf_lut(vec3<f32>(F0c), vec3<f32>(1.0), vec3<f32>(1.0), coat_NV, coat_roughness,
                         coat_ior, false).reflectance.x;
        c.coat_color = vec3<f32>(weight * coat_weight * r);
        weight *= max(1.0 - r * coat_weight, 0.0);
        if (!all(coat_tint == vec3<f32>(1.0))) {
            let tint = slab_transmittance_at_angle(coat_tint, NV, coat_ior);
            coat_tint = mix(vec3<f32>(1.0), tint, saturate(coat_weight));
        }
    } else {
        coat_tint = vec3<f32>(1.0);
    }

    // Emission
    c.emission = weight * coat_tint * s.emission * s.emission_strength;

    // Metallic
    let reflection_tint = specular_tint;
    var reflection_color = vec3<f32>(0.0);
    if (metallic > 0.0) {
        let F0 = clamped_base_color;
        let F82 = min(reflection_tint, vec3<f32>(1.0));
        let metallic_brdf = brdf_f82_tint_lut(F0, F82, NV, roughness, do_multiscatter);
        reflection_color = weight * metallic * metallic_brdf;
        weight *= max(1.0 - metallic, 0.0);
    }

    // Transmission
    c.refraction_color = vec3<f32>(0.0);
    c.refraction_roughness = roughness;
    if (transmission_weight > 0.0) {
        let F0 = vec3<f32>(F0_from_ior(ior)) * reflection_tint;
        let b = bsdf_lut(F0, vec3<f32>(1.0), sqrt(clamped_base_color), NV, roughness, ior,
                         do_multiscatter);
        c.refraction_color = weight * transmission_weight * b.transmittance * coat_tint;
        reflection_color += weight * transmission_weight * b.reflectance;
        weight *= max(1.0 - transmission_weight, 0.0);
    }

    // Specular
    {
        var eta = ior;
        var f0 = F0_from_ior(eta);
        if (specular_ior_level != 0.5) {
            f0 *= 2.0 * specular_ior_level;
            eta = ior_from_F0(f0);
            if (ior < 1.0) {
                eta = 1.0 / eta;
            }
        }
        let F0 = clamp(vec3<f32>(f0) * reflection_tint, vec3<f32>(0.0), vec3<f32>(1.0));
        let b = bsdf_lut(F0, vec3<f32>(1.0), vec3<f32>(0.0), NV, roughness, eta, do_multiscatter);
        c.reflection_color = (reflection_color + weight * b.reflectance) * coat_tint;
        c.reflection_N = N;
        c.reflection_roughness = roughness;
        weight *= max(1.0 - reduce_max(b.reflectance), 0.0);
    }

    // Diffuse
    c.diffuse_color = weight * base_color * coat_tint + sheen_data_color;
    c.diffuse_N = N;
    return c;
}

// ---------------------------------------------------------------------------------------
// Linearly transformed cosines (eevee_ltc_lib)

fn ltc_diffuse_sphere_integral(avg_dir_z: f32, form_factor: f32) -> f32 {
    return sample_lut(vec2<f32>(avg_dir_z * 0.5 + 0.5, form_factor), UTIL_DISK_INTEGRAL_LAYER).w;
}

fn solve_cubic(coefs_in: vec4<f32>) -> vec3<f32> {
    var coefs = coefs_in;
    coefs = vec4<f32>(coefs.xyz / coefs.w, coefs.w);
    coefs = vec4<f32>(coefs.x, coefs.yz / 3.0, coefs.w);
    let A = coefs.w;
    let B = coefs.z;
    let C = coefs.y;
    let D = coefs.x;
    let delta = vec3<f32>(-coefs.zy * coefs.zz + coefs.yx, dot(vec2<f32>(coefs.z, -coefs.y), coefs.xy));
    let discr = dot(vec2<f32>(4.0 * delta.x, -delta.y), delta.zy);
    let sqrt_discr = sqrt(clamp(discr, 0.0, FLT_MAX));
    var xlc: vec2<f32>;
    var xsc: vec2<f32>;
    {
        let C_a = delta.x;
        let D_a = -2.0 * B * delta.x + delta.y;
        let theta = atan2(sqrt_discr, -D_a) / 3.0;
        let s = 2.0 * sqrt(-C_a);
        let x_1a = s * cos(theta);
        let x_3a = s * cos(theta + (2.0 / 3.0) * PI);
        let xl = select(x_3a, x_1a, (x_1a + x_3a) > 2.0 * B);
        xlc = vec2<f32>(xl - B, A);
    }
    {
        let C_d = delta.z;
        let D_d = -D * delta.y + 2.0 * C * delta.z;
        let theta = atan2(D * sqrt_discr, -D_d) / 3.0;
        let s = 2.0 * sqrt(-C_d);
        let x_1d = s * cos(theta);
        let x_3d = s * cos(theta + (2.0 / 3.0) * PI);
        let xs = select(x_3d, x_1d, x_1d + x_3d < 2.0 * C);
        xsc = vec2<f32>(-D, xs + C);
    }
    let E = xlc.y * xsc.y;
    let F = -xlc.x * xsc.y - xlc.y * xsc.x;
    let G = xlc.x * xsc.x;
    let xmc = vec2<f32>(C * F - B * G, -B * F + C * E);
    var root = vec3<f32>(xsc.x / xsc.y, xmc.x / xmc.y, xlc.x / xlc.y);
    if (root.x < root.y && root.x < root.z) {
        root = root.yxz;
    } else if (root.z < root.x && root.z < root.y) {
        root = root.xzy;
    }
    return root;
}

fn edge_integral_vec(v1: vec3<f32>, v2: vec3<f32>) -> vec3<f32> {
    let x = dot(v1, v2);
    let y = abs(x);
    let a = 0.8543985 + (0.4965155 + 0.0145206 * y) * y;
    let b = 3.4175940 + (4.1616724 + y) * y;
    let v = a / b;
    let theta_sintheta = select(0.5 * inverseSqrt(max(1.0 - x * x, 1e-7)) - v, v, x > 0.0);
    return cross(v1, v2) * theta_sintheta;
}

// `from_up_axis` (Duff et al.): the columns of the returned matrix are the basis.
fn from_up_axis(up: vec3<f32>) -> mat3x3<f32> {
    let z_sign = select(-1.0, 1.0, up.z >= 0.0);
    let a = -1.0 / (z_sign + up.z);
    let b = up.x * up.y * a;
    return mat3x3<f32>(
        vec3<f32>(1.0 + z_sign * sqr(up.x) * a, z_sign * b, -z_sign * up.x),
        vec3<f32>(b, z_sign + sqr(up.y) * a, -up.y),
        up);
}

// Orthonormal basis around N (columns T1, T2, N), as `detail::tangent_basis`.
fn tangent_basis(N: vec3<f32>, V: vec3<f32>) -> mat3x3<f32> {
    let NV = dot(N, V);
    if (NV > 0.999999) {
        return from_up_axis(N);
    }
    let T1 = normalize(V - N * NV);
    let T2 = cross(N, T1);
    return mat3x3<f32>(T1, T2, N);
}

// `ltc_mat` packing: float3x3(float3(v.x, 0, v.y), float3(0, 1, 0), float3(v.z, 0, v.w)).
fn ltc_unpack(v: vec4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(vec3<f32>(v.x, 0.0, v.y), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(v.z, 0.0, v.w));
}

fn ltc_evaluate_quad(c_in: array<vec3<f32>, 4>, N: vec3<f32>, V: vec3<f32>, Minv_in: mat3x3<f32>) -> f32 {
    let T = tangent_basis(N, V);
    let Minv = Minv_in * transpose(T);
    var c = c_in;
    c[0] = normalize(Minv * c[0]);
    c[1] = normalize(Minv * c[1]);
    c[2] = normalize(Minv * c[2]);
    c[3] = normalize(Minv * c[3]);
    var avg_dir = edge_integral_vec(c[0], c[1]);
    avg_dir += edge_integral_vec(c[1], c[2]);
    avg_dir += edge_integral_vec(c[2], c[3]);
    avg_dir += edge_integral_vec(c[3], c[0]);
    let form_factor_inv = inverseSqrt(dot(avg_dir, avg_dir));
    let avg_dir_z = (avg_dir * form_factor_inv).z;
    let form_factor = saturate(1.0 / form_factor_inv);
    return form_factor * ltc_diffuse_sphere_integral(avg_dir_z, form_factor);
}

fn ltc_evaluate_disk(N: vec3<f32>, V: vec3<f32>, Minv: mat3x3<f32>, disk_points: array<vec3<f32>, 4>) -> f32 {
    let T = tangent_basis(N, V);
    let R = transpose(T);
    let L0 = R * disk_points[0];
    let L1 = R * disk_points[1];
    let L2 = R * disk_points[2];
    var C = 0.5 * (L0 + L2);
    var V1 = 0.5 * (L1 - L2);
    var V2 = 0.5 * (L1 - L0);
    C = Minv * C;
    V1 = Minv * V1;
    V2 = Minv * V2;

    let d11 = dot(V1, V1);
    let d22 = dot(V2, V2);
    let d12 = dot(V1, V2);
    var a: f32;
    var inv_b: f32;
    let threshold = 0.0007;
    if (abs(d12) / sqrt(d11 * d22) > threshold) {
        let tr = d11 + d22;
        var det = -d12 * d12 + d11 * d22;
        det = sqrt(det);
        let u = 0.5 * sqrt(tr - 2.0 * det);
        let w = 0.5 * sqrt(tr + 2.0 * det);
        var e_max = u + w;
        var e_min = u - w;
        e_max *= e_max;
        e_min *= e_min;
        var V1_: vec3<f32>;
        var V2_: vec3<f32>;
        if (d11 > d22) {
            V1_ = d12 * V1 + (e_max - d11) * V2;
            V2_ = d12 * V1 + (e_min - d11) * V2;
        } else {
            V1_ = d12 * V2 + (e_max - d22) * V1;
            V2_ = d12 * V2 + (e_min - d22) * V1;
        }
        a = 1.0 / e_max;
        inv_b = e_min;
        V1 = normalize(V1_);
        V2 = normalize(V2_);
    } else {
        a = 1.0 / d11;
        inv_b = d22;
        V1 *= sqrt(a);
        V2 *= inverseSqrt(inv_b);
    }

    var V3 = normalize(cross(V1, V2));
    if (dot(C, V3) < 0.0) {
        V3 *= -1.0;
    }
    let L = dot(V3, C);
    let inv_L = 1.0 / L;
    let x0 = dot(V1, C) * inv_L;
    let y0 = dot(V2, C) * inv_L;
    let ab = a * inv_b;
    inv_b *= sqr(inv_L);
    let t = 1.0 + x0 * x0;
    let c0 = ab * inv_b;
    let c1 = ab * (t + y0 * y0) - c0 - inv_b;
    let c2 = inv_b - ab * t - (1.0 + y0 * y0);
    let c3 = 1.0;
    let roots = solve_cubic(vec4<f32>(c0, c1, c2, c3));
    let e1 = roots.x;
    let e2 = roots.y;
    let e3 = roots.z;
    var avg_dir = vec3<f32>(ab * x0 / (ab - e2), y0 / (1.0 - e2), 1.0);
    let rotate = mat3x3<f32>(V1, V2, V3);
    avg_dir = normalize(rotate * avg_dir);
    let L1e = inverseSqrt(-e3 / e2);
    let L2e = inverseSqrt(-e1 / e2);
    let form_factor = saturate(L1e * L2e * inverseSqrt((1.0 + L1e * L1e) * (1.0 + L2e * L2e)));
    return form_factor * ltc_diffuse_sphere_integral(avg_dir.z, form_factor);
}

// ---------------------------------------------------------------------------------------
// Lights (eevee_light_lib)

fn is_sun(t: u32) -> bool { return t < LIGHT_OMNI_SPHERE; }
fn is_area(t: u32) -> bool { return t >= LIGHT_RECT; }
fn is_spot(t: u32) -> bool { return t == LIGHT_SPOT_SPHERE || t == LIGHT_SPOT_DISK; }
fn is_sphere(t: u32) -> bool { return t == LIGHT_SPOT_SPHERE || t == LIGHT_OMNI_SPHERE; }
fn is_oriented_disk(t: u32) -> bool { return t == LIGHT_SPOT_DISK || t == LIGHT_OMNI_DISK; }

struct LightVector {
    L: vec3<f32>,
    dist: f32,
};

fn light_vector_get(l: Light, directional: bool, P: vec3<f32>) -> LightVector {
    var lv: LightVector;
    if (directional) {
        lv.L = l.z_axis.xyz;
        lv.dist = 1.0;
    } else {
        let d = l.position.xyz - P;
        let inv_distance = inverseSqrt(dot(d, d));
        lv.L = d * inv_distance;
        lv.dist = 1.0 / inv_distance;
    }
    return lv;
}

fn light_world_to_local_direction(l: Light, L: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(l.x_axis.xyz, L), dot(l.y_axis.xyz, L), dot(l.z_axis.xyz, L));
}

fn light_influence_attenuation(dist: f32, inv_sqr_influence: f32) -> f32 {
    let factor = sqr(dist) * inv_sqr_influence;
    let fac = saturate(1.0 - sqr(factor));
    return sqr(fac);
}

fn light_spot_attenuation(l: Light, L: vec3<f32>) -> f32 {
    let lL = light_world_to_local_direction(l, L);
    let q = lL.xy * l.spot.xy / lL.z;
    let ellipse = inverseSqrt(1.0 + dot(q, q));
    let spotmask = smoothstep(0.0, 1.0, ellipse * l.spot.z + l.spot.w);
    return select(0.0, spotmask, lL.z > 0.0);
}

fn light_attenuation_common(l: Light, directional: bool, L: vec3<f32>) -> f32 {
    if (directional) {
        return 1.0;
    }
    let t = l.ints.x;
    if (is_spot(t)) {
        return light_spot_attenuation(l, L);
    }
    if (is_area(t)) {
        return select(0.0, 1.0, dot(L, l.z_axis.xyz) > 0.0);
    }
    return 1.0;
}

fn light_shape_radius(l: Light) -> f32 {
    let t = l.ints.x;
    if (is_sun(t)) {
        return l.shape.z;
    }
    if (is_area(t)) {
        return length(l.shape.xy);
    }
    return l.local.z;
}

fn light_attenuation_facing(l: Light, L: vec3<f32>, distance_to_light: f32, Ng: vec3<f32>) -> f32 {
    let sin_solid_angle = light_shape_radius(l) / distance_to_light;
    let sin_light_angle = dot(L, Ng);
    let dist = sin_solid_angle + sin_light_angle;
    return saturate((dist + 0.1) * 10.0);
}

fn light_attenuation_surface(l: Light, directional: bool, lv: LightVector) -> f32 {
    var result = light_attenuation_common(l, directional, lv.L);
    if (!directional) {
        result *= light_influence_attenuation(lv.dist, l.local.x);
    }
    return result;
}

fn light_sphere_disk_radius(sphere_radius: f32, distance_to_sphere: f32) -> f32 {
    return sphere_radius * inverseSqrt(max(1e-8, 1.0 - sqr(sphere_radius / distance_to_sphere)));
}

fn make_orthonormal_basis_T(N: vec3<f32>) -> vec3<f32> {
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(N.z) < 0.99999);
    return normalize(cross(up, N));
}

fn light_shape_corners(l: Light, lv: LightVector) -> array<vec3<f32>, 4> {
    var Px = l.x_axis.xyz;
    var Py = l.y_axis.xyz;
    var v: array<vec3<f32>, 4>;
    let t = l.ints.x;
    let Lc = lv.L * lv.dist;
    if (t == LIGHT_RECT) {
        let size = l.shape.xy;
        v[0] = Px * size.x + Py * -size.y;
        v[1] = Px * size.x + Py * size.y;
        v[2] = -v[0];
        v[3] = -v[1];
        v[0] += Lc;
        v[1] += Lc;
        v[2] += Lc;
        v[3] += Lc;
    } else {
        if (!is_area(t)) {
            Px = make_orthonormal_basis_T(lv.L);
            Py = cross(lv.L, Px);
        }
        var size: vec2<f32>;
        if (is_sphere(t)) {
            size = vec2<f32>(light_sphere_disk_radius(l.local.z, lv.dist));
        } else if (is_oriented_disk(t)) {
            size = vec2<f32>(l.local.z);
        } else if (is_sun(t)) {
            size = vec2<f32>(l.shape.z);
        } else {
            size = l.shape.xy;
        }
        v[0] = Px * -size.x + Py * -size.y;
        v[1] = Px * size.x + Py * -size.y;
        v[2] = -v[0];
        v[0] += Lc;
        v[1] += Lc;
        v[2] += Lc;
        v[3] = vec3<f32>(0.0);
    }
    return v;
}

fn light_ltc(l: Light, N: vec3<f32>, V: vec3<f32>, lv: LightVector, ltc_mat: vec4<f32>,
             vertices: array<vec3<f32>, 4>) -> f32 {
    let t = l.ints.x;
    if (is_sphere(t) && lv.dist < l.local.z) {
        return 1.0;
    }
    let Minv = ltc_unpack(ltc_mat);
    if (t == LIGHT_RECT) {
        return ltc_evaluate_quad(vertices, N, V, Minv);
    }
    return ltc_evaluate_disk(N, V, Minv, vertices);
}

// ---------------------------------------------------------------------------------------
// Shadows (eevee/shadows.rs): reverse depth, PCF with the comparison sampler.

// Map `map` lives in layer `map / tiles²`, tile `map % tiles²` (row major); taps clamp to the
// tile's texel centres, as `ClampToEdge` does for a map that fills its layer.
fn shadow_pcf(map: u32, uv: vec2<f32>, ref_depth: f32, radius: f32) -> f32 {
    let texel = frame.shadow.w;
    let tiles = max(u32(frame.shadow.z + 0.5), 1u);
    let per = tiles * tiles;
    let t = map % per;
    let layer = i32(map / per);
    let tile = vec2<f32>(f32(t % tiles), f32(t / tiles));
    let inv_tiles = 1.0 / f32(tiles);
    let lo = vec2<f32>(0.5 * texel);
    let hi = vec2<f32>(1.0 - 0.5 * texel);
    let ang = frame.shadow.x * 2.0 * PI;
    let rot = mat2x2<f32>(cos(ang), sin(ang), -sin(ang), cos(ang));
    var sum = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let o = rot * vec2<f32>(f32(i), f32(j)) * radius * texel;
            let st = (tile + clamp(uv + o, lo, hi)) * inv_tiles;
            sum += textureSampleCompareLevel(shadow_tx, shadow_samp, st, layer, ref_depth);
        }
    }
    return sum / 9.0;
}

fn shadow_lookup(layer: u32, P: vec3<f32>, radius: f32) -> f32 {
    let c = shadow_mats[layer] * vec4<f32>(P, 1.0);
    if (c.w <= 0.0) {
        return 1.0;
    }
    let n = c.xyz / c.w;
    let uv = vec2<f32>(n.x * 0.5 + 0.5, 0.5 - n.y * 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || n.z > 1.0 || n.z < 0.0) {
        return 1.0;
    }
    // Reverse depth: lit when the receiver is at least as close to the light as the occluder.
    return shadow_pcf(layer, uv, n.z * (1.0 + 2e-5) + 1e-6, radius);
}

fn shadow_eval(l: Light, P: vec3<f32>, Ng: vec3<f32>, L: vec3<f32>) -> f32 {
    let kind = l.ints.y;
    if (kind == SHADOW_NONE) {
        return 1.0;
    }
    let first = l.ints.z;
    let radius = max(l.shadow_params.x, 0.5);
    // Normal offset: a texel and a half along the geometric normal (towards the light).
    let n_off = select(-Ng, Ng, dot(Ng, L) >= 0.0);
    if (kind == SHADOW_CASCADES) {
        let depth = -(frame.view * vec4<f32>(P, 1.0)).z;
        var c = 3u;
        if (depth <= l.cascade_far.x) {
            c = 0u;
        } else if (depth <= l.cascade_far.y) {
            c = 1u;
        } else if (depth <= l.cascade_far.z) {
            c = 2u;
        }
        let texel_world = l.cascade_texel[c];
        let Po = P + n_off * texel_world * 1.5 * radius;
        return shadow_lookup(first + c, Po, radius);
    }
    let d = P - l.shadow_origin.xyz;
    let dist = length(d);
    let texel_world = l.shadow_origin.w * dist;
    let Po = P + n_off * texel_world * 1.5 * radius;
    if (kind == SHADOW_SPOT) {
        return shadow_lookup(first, Po, radius);
    }
    let dd = Po - l.shadow_origin.xyz;
    let a = abs(dd);
    var face = 0u;
    if (a.x >= a.y && a.x >= a.z) {
        face = select(1u, 0u, dd.x >= 0.0);
    } else if (a.y >= a.z) {
        face = select(3u, 2u, dd.y >= 0.0);
    } else {
        face = select(5u, 4u, dd.z >= 0.0);
    }
    return shadow_lookup(first + face, Po, radius);
}

// ---------------------------------------------------------------------------------------
// World lighting: a constant colour (WP 5.1) or the HDRI probe (WP 5.2's `WorldLighting`).

// The environment's direction a world direction looks up: the Mapping node's Z rotation,
// `Rz(rotation) · d` (`ibl_world_lookup`, `WorldLighting::rotation`).
fn world_rotate(d: vec3<f32>) -> vec3<f32> {
    let c = frame.world_params.x;
    let s = frame.world_params.y;
    return vec3<f32>(c * d.x - s * d.y, s * d.x + c * d.y, d.z);
}

// `sqrt_fast` (gpu_shader_math_fast_lib.glsl): Blender's bit-trick square root.
fn world_sqrt_fast(v: f32) -> f32 {
    return bitcast<f32>(0x1fbd1df5i + (bitcast<i32>(v) >> 1u));
}

// `roughness_to_lod` (SPHERE_PROBE_MIP_MAX_ROUGHNESS 0.7, linear UI roughness → probe mip).
fn world_roughness_to_lod(roughness: f32, mips: f32) -> f32 {
    let ratio = saturate(roughness / 0.7);
    let mip_ratio = mix(ratio, world_sqrt_fast(ratio), 0.4);
    return mip_ratio * mips;
}

// `roughness_to_mix_fac` (SPHERE_PROBE_MIX_START_ROUGHNESS 0.7, _END_ROUGHNESS 0.9).
fn world_roughness_to_mix_fac(roughness: f32) -> f32 {
    let scale = 1.0 / (0.9 - 0.7);
    let x = saturate(roughness * scale - scale * 0.7);
    return x * x;
}

// `bxdf_ggx_dominant_direction_reflection` (linear UI roughness).
fn world_dominant_direction(N: vec3<f32>, V: vec3<f32>, roughness: f32) -> vec3<f32> {
    let m = roughness * roughness;
    let R = -reflect(V, N);
    let smoothness = 1.0 - m;
    let fac = smoothness * (sqrt(smoothness) + m);
    return normalize(mix(N, R, fac));
}

// Irradiance E(N) from the L2 SH (Ramamoorthi & Hanrahan); diffuse radiance is albedo E / π.
fn world_sh_irradiance(N: vec3<f32>) -> vec3<f32> {
    let x = N.x;
    let y = N.y;
    let z = N.z;
    var e = frame.sh[0].rgb * 0.886227;
    e += frame.sh[1].rgb * (1.023328 * y);
    e += frame.sh[2].rgb * (1.023328 * z);
    e += frame.sh[3].rgb * (1.023328 * x);
    e += frame.sh[4].rgb * (0.858086 * x * y);
    e += frame.sh[5].rgb * (0.858086 * y * z);
    e += frame.sh[6].rgb * (0.247708 * (3.0 * z * z - 1.0));
    e += frame.sh[7].rgb * (0.858086 * x * z);
    e += frame.sh[8].rgb * (0.429043 * (x * x - y * y));
    return max(e, vec3<f32>(0.0));
}

fn world_diffuse(N: vec3<f32>) -> vec3<f32> {
    if ((frame.counts.w & FRAME_WORLD_HDRI) != 0u) {
        return world_sh_irradiance(N) * M_1_PI;
    }
    return frame.world_color.rgb;
}

// The world probe of a reflection closure (`LightprobeRenderData::eval`, world probe):
// `mix(probe(R, roughness_to_lod), evaluate_lambert(R), roughness_to_mix_fac)` at the GGX
// dominant direction `R` (`ibl_world_radiance`).
fn world_specular(N: vec3<f32>, V: vec3<f32>, roughness: f32) -> vec3<f32> {
    if ((frame.counts.w & FRAME_WORLD_HDRI) != 0u) {
        let mips = f32(max(frame.counts.z, 1u) - 1u);
        let R = world_dominant_direction(N, V, roughness);
        let lod = world_roughness_to_lod(roughness, mips);
        let probe = textureSampleLevel(world_spec_tx, linear_samp, world_rotate(R), lod).rgb *
                    frame.world_params.z;
        return mix(probe, world_sh_irradiance(R) * M_1_PI, world_roughness_to_mix_fac(roughness));
    }
    return frame.world_color.rgb;
}

fn world_background(dir: vec3<f32>) -> vec3<f32> {
    if ((frame.counts.w & FRAME_WORLD_HDRI) != 0u) {
        let d = world_rotate(normalize(dir));
        // Equirectangular, as Blender's Environment Texture (`ibl_equirect_uv`):
        // u = -atan2(y, x) / 2π + 0.5.
        let u = -atan2(d.y, d.x) / (2.0 * PI) + 0.5;
        let v = atan2(d.z, length(d.xy)) / PI + 0.5;
        return textureSampleLevel(world_bg_tx, linear_samp, vec2<f32>(u, 1.0 - v), 0.0).rgb *
               frame.world_params.z;
    }
    return frame.world_color.rgb;
}

// colorspace::brightness_clamp_max
fn brightness_clamp_max(c: vec3<f32>, limit: f32) -> vec3<f32> {
    if (limit <= 0.0) {
        return c;
    }
    let m = reduce_max(c);
    return select(c, c * (limit / m), m > limit);
}

// ---------------------------------------------------------------------------------------
// Fragment stage

fn view_vector(P: vec3<f32>) -> vec3<f32> {
    if ((frame.counts.w & FRAME_PERSPECTIVE) != 0u) {
        return normalize(frame.camera_pos.xyz - P);
    }
    return -frame.camera_fwd.xyz;
}

// Hashed alpha testing (Wyman and McGuire), ported from Blender 5.2.4's
// `eevee_transparency.bsl.hh` (GPL-2.0-or-later, WP 5.5): a threshold hashed from the world
// position at two log-discretised scales around `hash_scale` (0.1 in final renders: sub-pixel,
// so the TAA samples converge), mapped to a uniform distribution and jittered per sample by
// `hash_offset` (`frame.clamp.z`). Front and back faces at one pixel get independent
// thresholds, as in Blender.
//
// The per-cell hash is an integer `pcg3d` of the cell coordinates, not Blender's
// `fract(1e4 * sin(17 x + 0.1 y) …)`: `sin` of arguments in the thousands has no accuracy
// guarantee (WGSL bounds it on [-π, π] only) and the 1e4 factor turns its last bits into the
// threshold, so every adapter (Metal, lavapipe, WARP) drew a different noise pattern. The
// integer hash is bit-identical everywhere and as uniform; Blender's own pattern depends on
// its GPU's `sin` (deviations D-170).
fn hashed_transparency_pcg3d(v: vec3<u32>) -> vec3<u32> {
    var u = v * 1664525u + 1013904223u;
    u.x += u.y * u.z;
    u.y += u.z * u.x;
    u.z += u.x * u.y;
    u ^= u >> vec3<u32>(16u);
    u.x += u.y * u.z;
    u.y += u.z * u.x;
    u.z += u.x * u.y;
    return u;
}

// A uniform value in [0, 1) of the integral cell `a` (24 bits: exact in f32).
fn hashed_transparency_hash_3d(a: vec3<f32>) -> f32 {
    let u = hashed_transparency_pcg3d(bitcast<vec3<u32>>(vec3<i32>(a)));
    return f32(u.x >> 8u) * (1.0 / 16777216.0);
}

fn hashed_alpha_threshold(hash_scale: f32, hash_offset: f32, P: vec3<f32>, dPdx: vec3<f32>,
                          dPdy: vec3<f32>) -> f32 {
    let max_deriv = max(length(dPdx), length(dPdy));
    let pix_scale = 1.0 / (hash_scale * max_deriv);
    let pix_scale_log = log2(pix_scale);
    let pix_scales = vec2<f32>(exp2(floor(pix_scale_log)), exp2(ceil(pix_scale_log)));
    let alpha = vec2<f32>(hashed_transparency_hash_3d(floor(pix_scales.x * P)),
                          hashed_transparency_hash_3d(floor(pix_scales.y * P)));
    let fac = fract(log2(pix_scale));
    let x = mix(alpha.x, alpha.y, fac);
    let a = min(fac, 1.0 - fac);
    let one_a = 1.0 - a;
    let denom = 1.0 / (2.0 * a * one_a);
    let one_x = 1.0 - x;
    let cases = vec3<f32>((x * x) * denom, (x - 0.5 * a) / one_a, 1.0 - (one_x * one_x * denom));
    var threshold = select(cases.z, select(cases.y, cases.x, x < a), x < one_a);
    threshold = fract(threshold + hash_offset);
    return clamp(threshold, 1.0e-6, 1.0);
}

// Direct lighting of the closures by every light.
struct Lighting {
    diffuse: vec3<f32>,
    reflection: vec3<f32>,
    coat: vec3<f32>,
};

fn eval_lights(P: vec3<f32>, Ng: vec3<f32>, V: vec3<f32>, c: Closures) -> Lighting {
    var out: Lighting;
    out.diffuse = vec3<f32>(0.0);
    out.reflection = vec3<f32>(0.0);
    out.coat = vec3<f32>(0.0);
    let ltc_identity = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    let ltc_refl = sample_lut(vec2<f32>(c.reflection_roughness,
                                        sqrt(saturate(1.0 - dot(c.reflection_N, V)))), UTIL_LTC_MAT_LAYER);
    let has_coat = reduce_max(c.coat_color) > 0.0;
    let ltc_coat = sample_lut(vec2<f32>(c.coat_roughness, sqrt(saturate(1.0 - dot(c.coat_N, V)))),
                              UTIL_LTC_MAT_LAYER);
    let has_refl = reduce_max(c.reflection_color) > 0.0;
    let has_diff = reduce_max(c.diffuse_color) > 0.0;
    let n = frame.counts.x;
    for (var i = 0u; i < n; i++) {
        let l = lights[i];
        let directional = is_sun(l.ints.x);
        let lv = light_vector_get(l, directional, P);
        var attenuation = light_attenuation_surface(l, directional, lv);
        attenuation *= light_attenuation_facing(l, lv.L, lv.dist, c.diffuse_N);
        if (attenuation < LIGHT_ATTENUATION_THRESHOLD) {
            continue;
        }
        let shadow = shadow_eval(l, P, Ng, lv.L);
        let vis = shadow * attenuation;
        if (vis <= 0.0) {
            continue;
        }
        let vertices = light_shape_corners(l, lv);
        if (has_diff && l.power.x > 0.0) {
            let r = light_ltc(l, c.diffuse_N, V, lv, ltc_identity, vertices);
            out.diffuse += l.color.rgb * (r * l.power.x * vis);
        }
        if (has_refl && l.power.y > 0.0) {
            let r = light_ltc(l, c.reflection_N, V, lv, ltc_refl, vertices);
            out.reflection += l.color.rgb * (r * l.power.y * vis);
        }
        if (has_coat && l.power.y > 0.0) {
            let r = light_ltc(l, c.coat_N, V, lv, ltc_coat, vertices);
            out.coat += l.color.rgb * (r * l.power.y * vis);
        }
    }
    return out;
}

struct FragOut {
    @location(0) color: vec4<f32>,
};

// Set by `shade` for a fragment to discard; `fs_forward` discards at its very end (WP 5.5).
// A `discard` before the shading lets Metal's compiler sink the position derivatives past it,
// where the quad's discarded lanes no longer contribute: `Ng` (and everything lit with it)
// came out NaN next to DITHERED and alpha-clipped fragments.
var<private> frag_discard: bool;

fn shade(vin: VertexOut, front_facing: bool) -> vec4<f32> {
    // Derivatives first, in uniform control flow.
    let dPdx = dpdx(vin.world_pos);
    let dPdy = dpdy(vin.world_pos);
    var v: Inputs;
    v.duv0dx = dpdx(vin.uv0);
    v.duv0dy = dpdy(vin.uv0);
    v.duv1dx = dpdx(vin.uv1);
    v.duv1dy = dpdy(vin.uv1);
    let flip = draw.flags.x != 0u;
    let front = front_facing != flip;
    let m = material;
    if (!front && (m.ints.w & MAT_DOUBLE_SIDED) == 0u) {
        frag_discard = true;
    }
    v.P = vin.world_pos;
    v.V = view_vector(vin.world_pos);
    // Geometric normal from the position derivatives, facing the viewer.
    var Ng = normalize(cross(dPdx, dPdy));
    if (dot(Ng, v.V) < 0.0) {
        Ng = -Ng;
    }
    var N = safe_normalize(vin.world_nor);
    if (!front) {
        N = -N;
    }
    v.Ng = Ng;
    v.N = N;
    v.uv0 = vin.uv0;
    v.uv1 = vin.uv1;
    v.color = vin.color;
    v.tangent = vin.tangent;
    v.frag = vin.clip;
    v.front = front;

    var s = surface_init(&v);
    eevee_features_apply(&s, &v);

    // Transparency (`material.ints.y`): CLIP discards under the cutoff, DITHERED discards
    // stochastically (the TAA samples average to the blend), BLENDED keeps the alpha.
    let mode = m.ints.y;
    var alpha = saturate(s.alpha);
    if (mode == BLEND_OPAQUE) {
        alpha = 1.0;
    } else if (mode == BLEND_CLIP) {
        if (alpha < m.specular_tint.w) {
            frag_discard = true;
        }
        alpha = 1.0;
    } else if (mode == BLEND_DITHERED) {
        // `surf_depth`: discarded when the transparency exceeds the threshold.
        if (1.0 - alpha > hashed_alpha_threshold(0.1, frame.clamp.z, vin.world_pos, dPdx, dPdy)) {
            frag_discard = true;
        }
        alpha = 1.0;
    }

    if ((m.ints.w & MAT_UNLIT) != 0u) {
        return vec4<f32>(s.base_color * alpha, alpha);
    }

    let V = v.V;
    let c = principled(s, V, (m.ints.w & MAT_MULTISCATTER) != 0u);
    let direct = eval_lights(v.P, Ng, V, c);
    var radiance_direct = direct.diffuse * c.diffuse_color + direct.reflection * c.reflection_color +
                          direct.coat * c.coat_color;
    var radiance_indirect = world_diffuse(c.diffuse_N) * c.diffuse_color +
        world_specular(c.reflection_N, V, c.reflection_roughness) * c.reflection_color +
        world_specular(c.coat_N, V, c.coat_roughness) * c.coat_color;
    if (reduce_max(c.refraction_color) > 0.0) {
        if ((m.ints.w & MAT_SCREEN_REFRACTION) != 0u) {
            radiance_indirect += refraction_radiance(vin.clip.xy, c.refraction_roughness) * c.refraction_color;
        } else {
            // Two transmission events through the object's thickness (`eevee_forward_lib`:
            // the closure colour is applied twice).
            radiance_indirect += refraction_probe(s.N, V, c.refraction_roughness, max(s.ior, 1e-5)) *
                                 c.refraction_color * c.refraction_color;
        }
    }
    radiance_direct = brightness_clamp_max(radiance_direct, frame.clamp.x);
    radiance_indirect = brightness_clamp_max(radiance_indirect, frame.clamp.y);
    let radiance = radiance_direct + radiance_indirect + c.emission;
    return vec4<f32>(radiance * alpha, alpha);
}

// `bxdf_ggx_perceived_roughness_transmission` (eevee_bxdf_microfacet, WP 5.5).
fn refraction_perceived_roughness(roughness: f32, ior: f32) -> f32 {
    let x = select(saturate(1.0 - ior) * 0.64, 1.0 - 1.0 / ior, ior > 1.0);
    return roughness * world_sqrt_fast(x);
}

// `bxdf_ggx_dominant_direction_transmission`.
fn refraction_dominant_direction(N: vec3<f32>, V: vec3<f32>, ior: f32, roughness: f32) -> vec3<f32> {
    let m = roughness * roughness;
    let R = refract(-V, N, 1.0 / ior);
    let smoothness = 1.0 - m;
    let fac = smoothness * (sqrt(smoothness) + m);
    return normalize(mix(-N, R, fac));
}

// The world probe seen through a refraction closure (`LightProbe::eval` of a GGX refraction,
// the default without ray-traced refraction): `bxdf_ggx_context_amend_transmission` with the
// sphere thickness model (`Thickness::sphere_intersect`) moves the closure to the exit point,
// then `bxdf_ggx_lightprobe_transmission` refracts out (IOR inverted) and the probe is sampled
// like a reflection (`roughness_to_lod`, mixed with the SH by `roughness_to_mix_fac`).
fn refraction_probe(N: vec3<f32>, V: vec3<f32>, roughness: f32, ior: f32) -> vec3<f32> {
    let pr = refraction_perceived_roughness(roughness, ior);
    let L = refraction_dominant_direction(N, V, ior, pr);
    let cos_alpha = dot(L, -N);
    let hit_N = normalize(N + L * (cos_alpha * 2.0));
    let D = refraction_dominant_direction(-hit_N, -L, 1.0 / ior, pr);
    if ((frame.counts.w & FRAME_WORLD_HDRI) != 0u) {
        let mips = f32(max(frame.counts.z, 1u) - 1u);
        let lod = world_roughness_to_lod(pr, mips);
        let probe = textureSampleLevel(world_spec_tx, linear_samp, world_rotate(D), lod).rgb *
                    frame.world_params.z;
        return mix(probe, world_sh_irradiance(D) * M_1_PI, world_roughness_to_mix_fac(pr));
    }
    return frame.world_color.rgb;
}

// Screen-space refraction of the opaque pass (transmission): the opaque colour behind the
// fragment. Without the opaque copy (no refraction pass), the world.
fn refraction_radiance(frag: vec2<f32>, roughness: f32) -> vec3<f32> {
    if ((frame.counts.w & FRAME_REFRACTION) == 0u) {
        return frame.world_color.rgb;
    }
    let uv = frag * frame.resolution.zw;
    let c = textureSampleLevel(refraction_tx, linear_samp, uv, 0.0);
    return c.rgb;
}

@fragment
fn fs_forward(vin: VertexOut, @builtin(front_facing) front_facing: bool) -> FragOut {
    var o: FragOut;
    frag_discard = false;
    o.color = shade(vin, front_facing);
    if (frag_discard) {
        discard;
    }
    return o;
}

// ---------------------------------------------------------------------------------------
// Background (the world as the camera sees it, or transparent with `film_transparent`).

struct BgOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_background(@builtin(vertex_index) i: u32) -> BgOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: BgOut;
    o.ndc = uv * 2.0 - 1.0;
    // On the far plane (depth 1) so that surfaces cover it.
    o.clip = vec4<f32>(o.ndc, 1.0, 1.0);
    return o;
}

@fragment
fn fs_background(bg: BgOut) -> FragOut {
    var o: FragOut;
    if ((frame.counts.w & FRAME_FILM_TRANSPARENT) != 0u) {
        o.color = vec4<f32>(0.0);
        return o;
    }
    let a = frame.inv_view_proj * vec4<f32>(bg.ndc, 0.0, 1.0);
    let b = frame.inv_view_proj * vec4<f32>(bg.ndc, 1.0, 1.0);
    let dir = b.xyz / b.w - a.xyz / a.w;
    o.color = vec4<f32>(world_background(dir), 1.0);
    return o;
}
