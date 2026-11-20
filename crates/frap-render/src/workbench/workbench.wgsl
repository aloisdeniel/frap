// SPDX-License-Identifier: GPL-3.0-or-later
// frap Workbench uber-shader (ARCHITECTURE §19.2: one module, features are uniform flags, no
// per-material pipeline permutations). Entry points:
//
// - `vs_geom` + `fs_opaque`: opaque surfaces (forward-shaded G-buffer + resolve), writing the
//   lit colour, the colour in shadow and the object id;
// - `vs_geom` + `fs_transparent`: weighted blended order-independent transparency (X-ray and
//   materials with alpha < 1), `fs_object_id`: the transparent depth/id pass;
// - `vs_shadow`: stencil shadow volumes (depth-fail), no fragment stage;
// - `vs_full` + `fs_shadow_resolve` / `fs_transparent_resolve` / `fs_outline` /
//   `fs_taa_accumulate`: full-screen passes.
//
// Ported from Blender 5.2.4 `source/blender/draw/engines/workbench/shaders/`
// (GPL-2.0-or-later → GPL-3.0-or-later):
// - lighting: `workbench_world_light.bsl.hh` (`get_world_lighting`, `wrapped_lighting`,
//   `blinn_specular`, `brdf_approx`, `fast_rcp`, `get_shadow`);
// - matcaps: `workbench_matcap.bsl.hh` (`matcap_uv_compute`, `get_matcap_lighting`);
// - G-buffer: `workbench_prepass.bsl.hh` (`frag_opaque`: normals sphere-map encoded into a
//   half-float target, material into a half-float target), `workbench_common.bsl.hh`
//   (`normal_encode`/`normal_decode`), `workbench_composite.bsl.hh` (deferred resolve; the
//   view incident vector is the pixel ray, as `drw_point_screen_to_view(uv, 0.5)` gives);
// - transparency: `workbench_prepass.bsl.hh` (`frag_transparent`,
//   `calculate_transparent_weight`, no back-face normal flip),
//   `workbench_transparent_resolve_frag.glsl`;
// - outline: `workbench_effect_outline_frag.glsl`;
// - shadows: `workbench_shadow.bsl.hh` (volumes built on the CPU by `shadow.rs`; the depth bias
//   of the Metal backend, which Blender 5.2.4 uses on macOS, is applied here);
// - anti-aliasing: `workbench_effect_taa_frag.glsl` (3 x 3 neighbourhood in `log2(c + 1)`
//   space, half-float colour and accumulation targets).
//
// Blender evaluates the opaque lighting in a deferred resolve pass; evaluating it per fragment
// on the same quantised G-buffer values gives the same result for opaque surfaces.

const LIGHTING_STUDIO: u32 = 0u;
const LIGHTING_MATCAP: u32 = 1u;
const LIGHTING_FLAT: u32 = 2u;

// `Frame.mode.w` flags.
const FLAG_BACKFACE_CULLING: u32 = 1u;
const FLAG_MATCAP_FLIP: u32 = 2u;
const FLAG_TAA: u32 = 4u;
const FLAG_FIRST_SAMPLE: u32 = 8u;

struct Light {
    // xyz: direction (view space)
    direction: vec4<f32>,
    // rgb: specular colour
    specular_color: vec4<f32>,
    // rgb: diffuse colour, a: wrap (`smooth`)
    diffuse_color_wrap: vec4<f32>,
};

struct Frame {
    view: mat4x4<f32>,
    // Jittered projection (wgpu depth range).
    proj: mat4x4<f32>,
    lights: array<Light, 4>,
    ambient_color: vec4<f32>,
    // x: lighting mode, y: use_specular, z: is_perspective, w: FLAG_* bits
    mode: vec4<u32>,
    // x: shadow_shift, y: shadow_focus, z: shadow_mul, w: shadow_add (`ShadowPass::init`)
    shadow: vec4<f32>,
    // xyz: `shadow_direction_vs`
    shadow_direction: vec4<f32>,
    // x: xray_alpha, y: clip_start, z: clip_end
    params: vec4<f32>,
    // rgb: `object_outline_color`, a: 1
    outline_color: vec4<f32>,
    // TAA neighbourhood weights (`samplesWeights[9]`, index (x + 1) * 3 + (y + 1)).
    taa_weights: array<vec4<f32>, 3>,
};

struct Draw {
    model: mat4x4<f32>,
    // Object -> view normal matrix (upper 3x3 used).
    normal_matrix: mat4x4<f32>,
    // rgb: base colour, a: alpha (8-bit quantised, as `Material::pack_data`)
    base_color: vec4<f32>,
    // x: metallic, y: roughness (both already G-buffer quantised), z: 1 when the object -> view
    // transform mirrors (front faces wind clockwise), w: image alpha cutoff (TEXTURE colour,
    // `image_transparency_cutoff`: 0.1, or -1e30 without a cutoff)
    params: vec4<f32>,
    // x: roughness, y: metallic as the transparent pass sees them (8-bit packed only)
    params2: vec4<f32>,
    // x: object id (`resource_id + 1`, 0 is the background), y: colour source (`COLOR_*`),
    // z: `IMAGE_*` bits of the TEXTURE colour
    ids: vec4<u32>,
};

// `Draw.ids.y`: where the surface colour comes from (WP 5.5).
const COLOR_UNIFORM: u32 = 0u;
// VERTEX: the colour attribute (`material_attribute_color`, `ac`).
const COLOR_ATTRIBUTE: u32 = 1u;
// TEXTURE: the material's active image on the active UV map (`image_color`).
const COLOR_IMAGE: u32 = 2u;
// `Draw.ids.z` bits: the image's `extension` is CLIP (transparent black outside [0, 1]).
const IMAGE_CLIP: u32 = 1u;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var matcap_tx: texture_2d_array<f32>;
@group(0) @binding(2) var matcap_sampler: sampler;
@group(1) @binding(0) var<uniform> draw: Draw;
// The TEXTURE colour's image (straight alpha, linear) and its sampler; a 1 x 1 white image
// for the other draws (WP 5.5).
@group(2) @binding(0) var image_tx: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;

// Half-float storage of a value (G-buffer, colour and accumulation targets are half floats in
// Blender), rounded toward zero: what the Apple GPUs Blender's references were rendered on do
// when a render target converts to half (verified: the 8-sample TAA alpha of an opaque pixel
// comes out as 0.998046875, which only truncation gives). Done explicitly so every adapter
// stores the same values.
//
// Integer bit masking on the f32, not `unpack2x16float(pack2x16float(x))`: Mesa's NIR folds
// that round trip to `x` (an inexact algebraic rule), so on lavapipe nothing was quantised
// (verified on Mesa 25.2.8 llvmpipe: `unpack2x16float(pack2x16float(0.123457)).x == 0.123457`).
// Equals `color::round_f16_toward_zero` (CPU) for every input.
fn rtz16(x: f32) -> f32 {
    let u = bitcast<u32>(x);
    let sign = u & 0x80000000u;
    let a = u & 0x7fffffffu;
    if (a >= 0x7f800000u) {
        // Infinity and NaN are kept.
        return x;
    }
    if (a >= 0x477fe000u) {
        // At or above 65504, the largest half: truncation never reaches infinity.
        return bitcast<f32>(sign | 0x477fe000u);
    }
    let e = a >> 23u;
    if (e < 103u) {
        // Below 2^-24, the smallest half subnormal.
        return bitcast<f32>(sign);
    }
    // Half normals (f32 biased exponent >= 113) keep 10 mantissa bits (13 dropped); half
    // subnormals keep the bits down to 2^-24 (126 - e dropped, 14..=23).
    let drop = select(126u - e, 13u, e >= 113u);
    return bitcast<f32>(sign | (a & ~((1u << drop) - 1u)));
}

fn q16(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(rtz16(v.x), rtz16(v.y), rtz16(v.z), rtz16(v.w));
}

fn q16_2(v: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(rtz16(v.x), rtz16(v.y));
}

struct GeomOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal_vs: vec3<f32>,
    @location(1) position_vs: vec3<f32>,
    // The colour attribute (`ac`, linear) and the UV map (`au`).
    @location(2) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
};

@vertex
fn vs_geom(@location(0) pos: vec3<f32>, @location(1) nor: vec3<f32>,
           @location(2) color: vec4<f32>, @location(3) uv: vec2<f32>) -> GeomOut {
    var out: GeomOut;
    out.color = color;
    out.uv = uv;
    let world = draw.model * vec4<f32>(pos, 1.0);
    let view = frame.view * world;
    out.position = frame.proj * view;
    out.position_vs = view.xyz;
    let n = (draw.normal_matrix * vec4<f32>(nor, 0.0)).xyz;
    let len2 = dot(n, n);
    out.normal_vs = select(vec3<f32>(0.0, 0.0, 1.0), n * inverseSqrt(len2), len2 > 0.0);
    return out;
}

// [Drobot2014a] Low Level Optimizations for GCN (Blender `workbench_fast_rcp`).
fn fast_rcp(v: vec4<f32>) -> vec4<f32> {
    return bitcast<vec4<f32>>(vec4<i32>(0x7eef370b) - bitcast<vec4<i32>>(v));
}

fn brdf_approx(spec_color: vec3<f32>, roughness: f32, NV: f32) -> vec3<f32> {
    let fresnel = exp2(-8.35 * NV) * (1.0 - roughness);
    return mix(spec_color, vec3<f32>(1.0), fresnel);
}

fn safe_pow(x: vec4<f32>, y: vec4<f32>) -> vec4<f32> {
    // GLSL `pow(0, y > 0)` is 0 on every driver Blender supports; WGSL leaves it undefined.
    return select(pow(max(x, vec4<f32>(1e-30)), y), vec4<f32>(0.0), x <= vec4<f32>(0.0));
}

fn blinn_specular(shininess: vec4<f32>, spec_angle: vec4<f32>, NL: vec4<f32>) -> vec4<f32> {
    let normalization_factor = shininess * 0.125 + 1.0;
    return safe_pow(spec_angle, shininess) * NL * normalization_factor;
}

fn wrapped_lighting(NL: vec4<f32>, w: vec4<f32>) -> vec4<f32> {
    let w_1 = w + 1.0;
    let denom = fast_rcp(w_1 * w_1);
    return clamp((NL + w) * denom, vec4<f32>(0.0), vec4<f32>(1.0));
}

// (wrapped NL, specular angle, specular NL) of one light (Blender's per-light block).
fn specular_terms(L: vec3<f32>, I: vec3<f32>, N: vec3<f32>, R: vec3<f32>) -> vec3<f32> {
    let half_dir = normalize(L + I);
    return vec3<f32>(dot(L, R), saturate(dot(half_dir, N)), saturate(dot(L, N)));
}

fn get_world_lighting(base_color: vec3<f32>, roughness: f32, metallic: f32, N: vec3<f32>,
                      I: vec3<f32>) -> vec3<f32> {
    let use_specular = frame.mode.y != 0u;
    var specular_color: vec3<f32>;
    var diffuse_color: vec3<f32>;
    if (use_specular) {
        diffuse_color = mix(base_color, vec3<f32>(0.0), metallic);
        specular_color = mix(vec3<f32>(0.05), base_color, metallic);
    } else {
        diffuse_color = base_color;
        specular_color = vec3<f32>(0.0);
    }

    var specular_light = frame.ambient_color.rgb;
    var diffuse_light = frame.ambient_color.rgb;
    let wrap = vec4<f32>(frame.lights[0].diffuse_color_wrap.a,
                         frame.lights[1].diffuse_color_wrap.a,
                         frame.lights[2].diffuse_color_wrap.a,
                         frame.lights[3].diffuse_color_wrap.a);

    if (use_specular) {
        let R = -reflect(I, N);
        // Lights are evaluated four at a time with explicit components (no dynamic vector
        // component writes: friendlier to FXC on DX12).
        let t0 = specular_terms(frame.lights[0].direction.xyz, I, N, R);
        let t1 = specular_terms(frame.lights[1].direction.xyz, I, N, R);
        let t2 = specular_terms(frame.lights[2].direction.xyz, I, N, R);
        let t3 = specular_terms(frame.lights[3].direction.xyz, I, N, R);
        let wrapped_NL = vec4<f32>(t0.x, t1.x, t2.x, t3.x);
        let spec_angle = vec4<f32>(t0.y, t1.y, t2.y, t3.y);
        let spec_NL = vec4<f32>(t0.z, t1.z, t2.z, t3.z);
        var gloss = vec4<f32>(1.0 - roughness);
        // Reduce gloss for smooth light (simulate bigger light).
        gloss *= 1.0 - wrap;
        let shininess = exp2(10.0 * gloss + 1.0);
        var spec_light = blinn_specular(shininess, spec_angle, spec_NL);
        // Simulate environment light.
        let w = mix(wrap, vec4<f32>(1.0), roughness);
        let spec_env = wrapped_lighting(wrapped_NL, w);
        spec_light = mix(spec_light, spec_env, wrap * wrap);
        specular_light += spec_light.x * frame.lights[0].specular_color.rgb;
        specular_light += spec_light.y * frame.lights[1].specular_color.rgb;
        specular_light += spec_light.z * frame.lights[2].specular_color.rgb;
        specular_light += spec_light.w * frame.lights[3].specular_color.rgb;
        let NV = saturate(dot(N, I));
        specular_color = brdf_approx(specular_color, roughness, NV);
    }
    specular_light *= specular_color;

    let diff_NL = vec4<f32>(dot(frame.lights[0].direction.xyz, N),
                            dot(frame.lights[1].direction.xyz, N),
                            dot(frame.lights[2].direction.xyz, N),
                            dot(frame.lights[3].direction.xyz, N));
    let diff_light = wrapped_lighting(diff_NL, wrap);
    diffuse_light += diff_light.x * frame.lights[0].diffuse_color_wrap.rgb;
    diffuse_light += diff_light.y * frame.lights[1].diffuse_color_wrap.rgb;
    diffuse_light += diff_light.z * frame.lights[2].diffuse_color_wrap.rgb;
    diffuse_light += diff_light.w * frame.lights[3].diffuse_color_wrap.rgb;
    // Energy conservation with coloured specular looks strange; Blender limits it with a
    // monochromatic specular intensity.
    let spec_energy = dot(specular_color, vec3<f32>(0.33333));
    diffuse_light *= diffuse_color * (1.0 - spec_energy);
    return diffuse_light + specular_light;
}

fn matcap_uv_compute(I: vec3<f32>, N: vec3<f32>, flipped: bool) -> vec2<f32> {
    // Quick creation of an orthonormal basis.
    let a = 1.0 / (1.0 + I.z);
    let b = -I.x * I.y * a;
    let b1 = vec3<f32>(1.0 - I.x * I.x * a, b, -I.x);
    let b2 = vec3<f32>(b, 1.0 - I.y * I.y * a, -I.y);
    var matcap_uv = vec2<f32>(dot(b1, N), dot(b2, N));
    if (flipped) {
        matcap_uv.x = -matcap_uv.x;
    }
    return matcap_uv * 0.496 + 0.5;
}

fn get_matcap_lighting(base_color: vec3<f32>, N: vec3<f32>, I: vec3<f32>) -> vec3<f32> {
    let flipped = (frame.mode.w & FLAG_MATCAP_FLIP) != 0u;
    let uv = matcap_uv_compute(I, N, flipped);
    let diffuse = textureSampleLevel(matcap_tx, matcap_sampler, uv, 0, 0.0).rgb;
    let specular = textureSampleLevel(matcap_tx, matcap_sampler, uv, 1, 0.0).rgb;
    return diffuse * base_color + specular * f32(frame.mode.y != 0u);
}

// `get_shadow(N, force_shadow)`: the shading of the shadow pass (1 when shadows are off:
// `shadow_mul = 0`, `shadow_add = 1`).
fn get_shadow(N: vec3<f32>, force_shadow: bool) -> f32 {
    let light_factor = -dot(N, frame.shadow_direction.xyz);
    var shadow_mix = smoothstep(frame.shadow.x, frame.shadow.y, light_factor);
    shadow_mix *= select(frame.shadow.z, 0.0, force_shadow);
    return shadow_mix + frame.shadow.w;
}

// The view incident vector of a fragment (`drw_view_incident_vector`).
fn incident(position_vs: vec3<f32>) -> vec3<f32> {
    if (frame.mode.z != 0u) {
        return normalize(-position_vs);
    }
    return vec3<f32>(0.0, 0.0, 1.0);
}

fn shade(base_color: vec3<f32>, roughness: f32, metallic: f32, N: vec3<f32>, I: vec3<f32>)
    -> vec3<f32> {
    let lighting = frame.mode.x;
    if (lighting == LIGHTING_STUDIO) {
        return get_world_lighting(base_color, roughness, metallic, N, I);
    }
    if (lighting == LIGHTING_MATCAP) {
        return get_matcap_lighting(base_color, N, I);
    }
    return base_color;
}

// Front facing in the object's winding (mirrored transforms flip it).
fn is_front(front_facing: bool) -> bool {
    return front_facing != (draw.params.z > 0.5);
}

fn cull(front: bool) {
    if (!front && (frame.mode.w & FLAG_BACKFACE_CULLING) != 0u) {
        discard;
    }
}

// `normal_encode` (sphere map) into a half-float target, then `normal_decode`.
fn gbuffer_normal(front: bool, n_in: vec3<f32>) -> vec3<f32> {
    var n = normalize(select(-n_in, n_in, front));
    let p = sqrt(n.z * 8.0 + 8.0);
    let enc = q16_2(clamp(n.xy / p + 0.5, vec2<f32>(0.0), vec2<f32>(1.0)));
    let fenc = enc * 4.0 - 2.0;
    let f = dot(fenc, fenc);
    let g = sqrt(1.0 - f / 4.0);
    return vec3<f32>(fenc * g, 1.0 - f / 2.0);
}

struct OpaqueOut {
    // Lit colour.
    @location(0) color: vec4<f32>,
    // Colour in the stencil shadow (`get_shadow(N, true)`).
    @location(1) shadowed: vec4<f32>,
    @location(2) object_id: u32,
};

// `workbench::color::image_color` (workbench_image.bsl.hh): the image at the UV (Blender's
// UVs have v up, the image rows are stored top first), straight alpha; fragments under the
// alpha cutoff are discarded.
fn image_color(uv_in: vec2<f32>) -> vec3<f32> {
    let uv = vec2<f32>(uv_in.x, 1.0 - uv_in.y);
    var color = textureSampleLevel(image_tx, image_sampler, uv, 0.0);
    if ((draw.ids.z & IMAGE_CLIP) != 0u &&
        (any(uv_in < vec2<f32>(0.0)) || any(uv_in > vec2<f32>(1.0)))) {
        color = vec4<f32>(0.0);
    }
    if (color.a < draw.params.w) {
        discard;
    }
    return color.rgb;
}

// The surface colour of a fragment: the material/object/single colour, the colour attribute
// (`material_data_get` with the -1 marker) or the image (WP 5.5).
fn surface_color(in: GeomOut) -> vec3<f32> {
    let source = draw.ids.y;
    if (source == COLOR_ATTRIBUTE) {
        return in.color.rgb;
    }
    if (source == COLOR_IMAGE) {
        return image_color(in.uv);
    }
    return draw.base_color.rgb;
}

@fragment
fn fs_opaque(in: GeomOut, @builtin(front_facing) front_facing: bool) -> OpaqueOut {
    let front = is_front(front_facing);
    cull(front);
    var N = gbuffer_normal(front, in.normal_vs);
    if (frame.mode.x == LIGHTING_MATCAP && !front) {
        // With matcaps the G-buffer alpha holds the facing and the resolve flips the normal
        // back (`N = (mat_data.a > 0.0f) ? N : -N`).
        N = -N;
    }
    let I = incident(in.position_vs);
    // `material_tx` is a half-float target.
    let base_color = q16(vec4<f32>(surface_color(in), 0.0)).rgb;
    let color = shade(base_color, draw.params.y, draw.params.x, N, I);
    var out: OpaqueOut;
    out.color = q16(vec4<f32>(color * get_shadow(N, false), 1.0));
    out.shadowed = q16(vec4<f32>(color * get_shadow(N, true), 1.0));
    out.object_id = draw.ids.x;
    return out;
}

// `calculate_transparent_weight` (McGuire and Bavoil 2013, eq. 7) of a fragment at view-space
// depth `z_vs` (`linear_zdepth`: the view distance in perspective, the distance from the near
// plane in orthographic projections).
fn transparent_weight(z_vs: f32) -> f32 {
    var z = abs(z_vs);
    if (frame.mode.z == 0u) {
        z = abs(-z_vs - frame.params.y);
    }
    let a = z / 5.0;
    var b = z / 200.0;
    b *= b;
    let w = 10.0 / ((1e-5 + a * a) + b * (b * b));
    return clamp(w, 1e-2, 3e2);
}

struct TransparentOut {
    @location(0) accum: vec4<f32>,
    @location(1) revealage: vec4<f32>,
};

@fragment
fn fs_transparent(in: GeomOut, @builtin(front_facing) front_facing: bool) -> TransparentOut {
    cull(is_front(front_facing));
    // No back-face flip in the transparent pass.
    let N = normalize(in.normal_vs);
    let I = incident(in.position_vs);
    var shaded = shade(surface_color(in), draw.params2.x, draw.params2.y, N, I);
    shaded *= get_shadow(N, false);
    let alpha = draw.base_color.a * frame.params.x;
    let weight = transparent_weight(in.position_vs.z) * alpha;
    var out: TransparentOut;
    out.accum = vec4<f32>(shaded * weight, alpha);
    out.revealage = vec4<f32>(weight);
    return out;
}

@fragment
fn fs_object_id(@builtin(front_facing) front_facing: bool) -> @location(0) u32 {
    cull(is_front(front_facing));
    return draw.ids.x;
}

// Shadow volume vertices, already extruded in world space by `shadow.rs`.
@vertex
fn vs_shadow(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    var p = frame.proj * (frame.view * vec4<f32>(pos, 1.0));
    // Blender's Metal backend biases shadow volumes (`out_pos.z += 0.00005` in OpenGL clip
    // space, half of it in wgpu's [0, w] depth range) against z-fighting with the caps.
    p.z += 0.000025;
    // The bias must not push back caps (extruded to just inside the far plane) out of the
    // view volume: a clipped cap breaks the depth-fail count.
    if (p.w > 0.0) {
        p.z = min(p.z, p.w);
    }
    return p;
}

struct FullOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> FullOut {
    // One triangle covering the viewport.
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: FullOut;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

// The shadow resolve triangle sits on the far plane: with `depth_compare: Greater` only pixels
// that hold geometry (depth < 1) pass, as Blender applies the shadow in its deferred resolve
// of the opaque surfaces only. Depth-fail volumes leave non-zero counts on background pixels
// too (back caps sit on the far plane, the cleared depth).
@vertex
fn vs_full_far(@builtin(vertex_index) i: u32) -> FullOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: FullOut;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 1.0, 1.0);
    return out;
}

@group(2) @binding(0) var tex_a: texture_2d<f32>;
@group(2) @binding(1) var tex_b: texture_2d<f32>;
@group(2) @binding(2) var tex_ids: texture_2d<u32>;

// Shadowed geometry pixels (stencil != 0 and depth < 1, tested by the pipeline) take the
// shadowed colour.
@fragment
fn fs_shadow_resolve(in: FullOut) -> @location(0) vec4<f32> {
    return textureLoad(tex_a, vec2<i32>(in.position.xy), 0);
}

@fragment
fn fs_transparent_resolve(in: FullOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    // Half-float accumulation targets.
    let trans_accum = q16(textureLoad(tex_a, p, 0));
    let trans_weight = q16(textureLoad(tex_b, p, 0)).r;
    let trans_reveal = trans_accum.a;
    return vec4<f32>(trans_accum.rgb / clamp(trans_weight, 1e-4, 5e4), 1.0 - trans_reveal);
}

fn object_id_at(p: vec2<i32>) -> u32 {
    let size = vec2<i32>(textureDimensions(tex_ids));
    return textureLoad(tex_ids, clamp(p, vec2<i32>(0), size - 1), 0).r;
}

@fragment
fn fs_outline(in: FullOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let center_id = object_id_at(p);
    let adjacent_ids = vec4<u32>(object_id_at(p + vec2<i32>(0, 1)),
                                 object_id_at(p - vec2<i32>(0, 1)),
                                 object_id_at(p + vec2<i32>(1, 0)),
                                 object_id_at(p - vec2<i32>(1, 0)));
    let same = vec4<f32>(adjacent_ids == vec4<u32>(center_id));
    let outline_opacity = 1.0 - dot(same, vec4<f32>(0.25));
    return frame.outline_color * outline_opacity;
}

fn color_at(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(tex_b));
    // The colour target is a half-float texture.
    return q16(textureLoad(tex_b, clamp(p, vec2<i32>(0), size - 1), 0));
}

fn taa_weight(i: u32) -> f32 {
    return frame.taa_weights[i / 4u][i % 4u];
}

@fragment
fn fs_taa_accumulate(in: FullOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    var sample_value = vec4<f32>(0.0);
    if ((frame.mode.w & FLAG_TAA) != 0u) {
        var i = 0u;
        for (var x = -1; x <= 1; x++) {
            for (var y = -1; y <= 1; y++) {
                // Blender's texel space is bottom-up: its +y neighbour is the row above.
                var color = color_at(p + vec2<i32>(x, -y));
                // Clamp infinite inputs (Blender #112211).
                color = clamp(color, vec4<f32>(0.0), vec4<f32>(1e10));
                // Use log2 space to avoid highlights creating too much aliasing.
                color = vec4<f32>(log2(color.rgb + 1.0), color.a);
                sample_value += color * taa_weight(i);
                i++;
            }
        }
    } else {
        // `render_aa = 'OFF'`: the colour target as is.
        sample_value = color_at(p);
    }
    if ((frame.mode.w & FLAG_FIRST_SAMPLE) != 0u) {
        return q16(sample_value);
    }
    // Additive blending into a half-float accumulation target.
    return q16(textureLoad(tex_a, p, 0) + sample_value);
}
