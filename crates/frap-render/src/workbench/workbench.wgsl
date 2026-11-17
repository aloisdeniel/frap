// SPDX-License-Identifier: GPL-3.0-or-later
// frap Workbench uber-shader (ARCHITECTURE §19.2: one module, features are uniform flags, no
// per-material pipeline permutations). Entry points: `vs_geom`/`fs_geom` (opaque surfaces,
// forward-shaded), `vs_full`/`fs_accum` (Rgba32Float sample accumulation).
//
// Lighting is ported from Blender 5.2.4 `source/blender/draw/engines/workbench/shaders/
// workbench_world_light.bsl.hh` (`workbench_get_world_lighting`, `workbench_wrapped_lighting`,
// `workbench_blinn_specular`, `workbench_brdf_approx`, `workbench_fast_rcp`) and
// `workbench_composite.bsl.hh` (`workbench_resolve_frag`); normals follow
// `workbench_prepass.bsl.hh` (`normalize(normal_object_to_view(nor))`, flipped on back faces
// as `workbench_normal_encode(gl_FrontFacing, …)` does). Blender evaluates the lighting in a
// deferred resolve pass; evaluating it per fragment gives the same value for opaque surfaces.
// GPL-2.0-or-later (Blender) -> GPL-3.0-or-later (frap).

const LIGHTING_STUDIO: u32 = 0u;
const LIGHTING_FLAT: u32 = 2u;

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
    // x: lighting mode, y: use_specular, z: is_perspective
    mode: vec4<u32>,
    // x: accumulation weight of this sample
    accum: vec4<f32>,
};

struct Draw {
    model: mat4x4<f32>,
    // Object -> view normal matrix (upper 3x3 used).
    normal_matrix: mat4x4<f32>,
    // rgb: base colour, a: alpha
    base_color: vec4<f32>,
    // x: metallic, y: roughness (both already G-buffer quantised), z: 1 when the object -> view
    // transform mirrors (front faces wind clockwise)
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;

struct GeomOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal_vs: vec3<f32>,
    @location(1) position_vs: vec3<f32>,
};

@vertex
fn vs_geom(@location(0) pos: vec3<f32>, @location(1) nor: vec3<f32>) -> GeomOut {
    var out: GeomOut;
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

@fragment
fn fs_geom(in: GeomOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let front = front_facing != (draw.params.z > 0.5);
    var N = normalize(in.normal_vs);
    if (!front) {
        N = -N;
    }
    var I = vec3<f32>(0.0, 0.0, 1.0);
    if (frame.mode.z != 0u) {
        I = normalize(-in.position_vs);
    }
    let base_color = draw.base_color.rgb;
    var color = base_color;
    if (frame.mode.x == LIGHTING_STUDIO) {
        color = get_world_lighting(base_color, draw.params.y, draw.params.x, N, I);
    }
    return vec4<f32>(color, 1.0);
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

@group(0) @binding(1) var accum_prev: texture_2d<f32>;
@group(0) @binding(2) var sample_tex: texture_2d<f32>;

@fragment
fn fs_accum(in: FullOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let prev = textureLoad(accum_prev, p, 0);
    let s = textureLoad(sample_tex, p, 0);
    // Premultiplied accumulation; the CPU un-premultiplies once at the end.
    return prev + vec4<f32>(s.rgb * s.a, s.a) * frame.accum.x;
}
