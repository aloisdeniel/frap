// SPDX-License-Identifier: GPL-3.0-or-later
// World probe bake (WP 5.2), prepended with common.wgsl. Ported from
// blender/source/blender/draw/engines/eevee/shaders/eevee_lightprobe_sphere_bake.bsl.hh
// (`remap_cubemap_to_octahedral`, `mip_convolve`) and eevee_lightprobe_sphere.bsl.hh; the
// world itself is drawn into a cube map first, as EEVEE renders the world shader into
// `cubemap_tx_` (`SphereProbeModule::ensure_cubemap_render_target`).
//
// Entry points (one pipeline each, auto layouts):
// - `world_to_cube`: the environment texture (unrotated, strength 1) into the 6 cube faces;
// - `remap`: cube → octahedral mip 0, splitting the radiance above `sun_threshold` into the
//   world sun, clamping by `clamp_surface_indirect`, and writing one partial sum per work
//   group (L1 spherical harmonics of the clamped radiance, sun radiance, sun direction);
// - `convolve`: octahedral mip `read_lod` → `read_lod + 1` with spherical-gaussian weights;
// - `octahedral_to_cube`: one octahedral mip into the matching mip of the specular cube map.

struct Params {
    // world_to_cube / octahedral_to_cube: cube face size of this mip.
    face_size: i32,
    // remap / convolve / octahedral_to_cube: octahedral extent of mip 0.
    mip0_extent: i32,
    // convolve: mip read; octahedral_to_cube: mip converted.
    read_lod: i32,
    // remap: work groups per row.
    groups_x: i32,
    _unused: f32,
    // remap: `uniform_buf.clamp.sun_threshold` (1e20 when off), in units of the environment
    // texture (divided by |strength|: the bake works at strength 1).
    sun_threshold: f32,
    // remap: `uniform_buf.clamp.surface_indirect` (1e20 when off), divided by |strength|.
    clamp_indirect: f32,
    _pad: f32,
}

struct Partial {
    sh_l0: vec4<f32>,
    sh_l1n1: vec4<f32>,
    sh_l10: vec4<f32>,
    sh_l1p1: vec4<f32>,
    sun_radiance: vec4<f32>,
    sun_direction: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var lin_sampler: sampler;
@group(0) @binding(2) var equirect_tx: texture_2d<f32>;
@group(0) @binding(3) var cube_out: texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(4) var cube_tx: texture_cube<f32>;
@group(0) @binding(5) var oct_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var<storage, read_write> partials: array<Partial>;
@group(0) @binding(7) var oct_in: texture_2d<f32>;

// Direction of texel centre `uv` (in [-1, 1], y down) on cube face `face` (the sampling
// convention of cube textures: +X, -X, +Y, -Y, +Z, -Z).
fn cube_direction(face: u32, u: f32, v: f32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(1.0, -v, -u); }
        case 1u: { return vec3<f32>(-1.0, -v, u); }
        case 2u: { return vec3<f32>(u, 1.0, v); }
        case 3u: { return vec3<f32>(u, -1.0, -v); }
        case 4u: { return vec3<f32>(u, -v, 1.0); }
        default: { return vec3<f32>(-u, -v, -1.0); }
    }
}

fn cube_texel_direction(id: vec3<u32>) -> vec3<f32> {
    let size = f32(params.face_size);
    let u = (f32(id.x) + 0.5) / size * 2.0 - 1.0;
    let v = (f32(id.y) + 0.5) / size * 2.0 - 1.0;
    return normalize(cube_direction(id.z, u, v));
}

@compute @workgroup_size(8, 8, 1)
fn world_to_cube(@builtin(global_invocation_id) id: vec3<u32>) {
    if (i32(id.x) >= params.face_size || i32(id.y) >= params.face_size) {
        return;
    }
    let dir = cube_texel_direction(id);
    let tc = ibl_equirect_tex_coord(dir, 0.0);
    let c = textureSampleLevel(equirect_tx, lin_sampler, tc, 0.0).rgb;
    textureStore(cube_out, vec2<i32>(id.xy), i32(id.z), vec4<f32>(c, 1.0));
}

fn triangle_solid_angle(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>) -> f32 {
    return 2.0 * atan2(abs(dot(a, cross(b, c))), 1.0 + dot(b, c) + dot(a, c) + dot(a, b));
}

fn quad_solid_angle(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> f32 {
    return triangle_solid_angle(a, b, c) + triangle_solid_angle(c, b, d);
}

// `octahedral_texel_solid_angle`
fn octahedral_texel_solid_angle(local_texel: vec2<i32>, extent: i32) -> f32 {
    if (local_texel.x == extent - 1 || local_texel.y == extent - 1) {
        return 0.0;
    }
    let half_size = extent >> 1u;
    let padded_size = extent - 2;
    var wrapped = local_texel;
    if (local_texel.x >= half_size) {
        wrapped.x = padded_size - local_texel.x;
    }
    if (local_texel.y >= half_size) {
        wrapped.y = padded_size - local_texel.y;
    }
    let w = vec2<f32>(wrapped);
    let hi = vec2<f32>(f32(half_size - 1));
    let c00 = clamp(w + vec2<f32>(-0.5, -0.5), vec2<f32>(0.0), hi);
    let c10 = clamp(w + vec2<f32>(0.5, -0.5), vec2<f32>(0.0), hi);
    let c01 = clamp(w + vec2<f32>(-0.5, 0.5), vec2<f32>(0.0), hi);
    let c11 = clamp(w + vec2<f32>(0.5, 0.5), vec2<f32>(0.0), hi);
    let v00 = normalize(ibl_texel_to_direction(c00, extent));
    let v10 = normalize(ibl_texel_to_direction(c10, extent));
    let v01 = normalize(ibl_texel_to_direction(c01, extent));
    let v11 = normalize(ibl_texel_to_direction(c11, extent));
    var sa = quad_solid_angle(v00, v10, v01, v11);
    if (wrapped.x == half_size - 1) {
        sa *= 2.0;
    }
    if (wrapped.y == half_size - 1) {
        sa *= 2.0;
    }
    return sa;
}

var<workgroup> local_sum: array<vec4<f32>, 256>;

fn reduce(index: u32, value: vec4<f32>) -> vec4<f32> {
    local_sum[index] = value;
    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        workgroupBarrier();
        if (index < stride) {
            local_sum[index] = local_sum[index] + local_sum[index + stride];
        }
    }
    workgroupBarrier();
    let total = local_sum[0];
    workgroupBarrier();
    return total;
}

@compute @workgroup_size(16, 16, 1)
fn remap(@builtin(global_invocation_id) gid: vec3<u32>,
         @builtin(workgroup_id) group: vec3<u32>,
         @builtin(local_invocation_index) local_index: u32) {
    let extent = params.mip0_extent;
    let texel = vec2<i32>(gid.xy);
    let inside = texel.x < extent && texel.y < extent;
    let direction = ibl_texel_to_direction(vec2<f32>(texel), extent);
    var radiance = vec3<f32>(0.0);
    var weight = 0.0;
    if (inside) {
        radiance = textureSampleLevel(cube_tx, lin_sampler, direction, 0.0).rgb;
        weight = octahedral_texel_solid_angle(texel, extent);
    }
    let clamped = ibl_brightness_clamp_max(radiance, params.sun_threshold);
    let radiance_sun = radiance - clamped;
    radiance = clamped;
    if (inside) {
        let out_radiance = ibl_brightness_clamp_max(radiance, params.clamp_indirect);
        textureStore(oct_out, texel, vec4<f32>(out_radiance, 1.0));
    }
    let l = normalize(direction);
    let amp = vec4<f32>(radiance * weight, 0.0);
    // SphericalHarmonicL1::encode_signal_sample, per texel.
    let sh_l0 = reduce(local_index, 0.282094792 * amp);
    let sh_l1n1 = reduce(local_index, -0.488602512 * l.y * amp);
    let sh_l10 = reduce(local_index, 0.488602512 * l.z * amp);
    let sh_l1p1 = reduce(local_index, -0.488602512 * l.x * amp);
    let sun_radiance = reduce(local_index, vec4<f32>(radiance_sun * weight, 0.0));
    let sun_direction = reduce(local_index,
                               vec4<f32>(l, 1.0) * weight * length(radiance_sun));
    if (local_index == 0u) {
        let g = group.y * u32(params.groups_x) + group.x;
        partials[g] = Partial(sh_l0, sh_l1n1, sh_l10, sh_l1p1, sun_radiance, sun_direction);
    }
}

// `roughness_from_relative_mip`
fn roughness_from_relative_mip(prev_mip_roughness: f32, curr_mip_roughness: f32) -> f32 {
    let exponent = 3.0;
    let m_prev = pow(prev_mip_roughness, exponent);
    let m_curr = pow(curr_mip_roughness, exponent);
    let m_target = m_curr - m_prev;
    return pow(m_target, 1.0 / exponent);
}

// `cone_cosine_from_roughness`
fn cone_cosine_from_roughness(linear_roughness: f32) -> f32 {
    let m = linear_roughness * linear_roughness;
    let cutoff_value = mix(0.01, 0.14, m);
    let half_angle_cos = 1.0 + (log(cutoff_value) * m * m) / 2.0;
    let half_angle_sin = sqrt(max(0.0, 1.0 - half_angle_cos * half_angle_cos));
    return half_angle_cos * half_angle_cos - half_angle_sin * half_angle_sin;
}

// `sample_weight`
fn sample_weight(out_direction: vec3<f32>, in_direction: vec3<f32>, linear_roughness: f32) -> f32 {
    let o = normalize(out_direction);
    let i = normalize(in_direction);
    let m = linear_roughness * linear_roughness;
    let h = normalize(o + i);
    let nh = saturate(dot(o, h));
    return exp(2.0 * (nh - 1.0) / (m * m));
}

// `direction_to_uv` (single mip, probe filling its texture).
fn direction_to_uv(l: vec3<f32>, lod: f32, mip0_res: i32) -> vec2<f32> {
    return ibl_miplvl_scale_bias(lod, mip0_res, ibl_octahedral_uv_from_direction(l));
}

// BSDF_ROUGHNESS_THRESHOLD
const ROUGHNESS_THRESHOLD: f32 = 2e-2;
const CONVOLVE_SAMPLES: u32 = 196u;

@compute @workgroup_size(16, 16, 1)
fn convolve(@builtin(global_invocation_id) gid: vec3<u32>) {
    let out_extent = params.mip0_extent >> u32(params.read_lod + 1);
    let texel = vec2<i32>(gid.xy);
    if (texel.x >= out_extent || texel.y >= out_extent) {
        return;
    }
    let prev_mip_roughness = ibl_lod_to_roughness(f32(params.read_lod));
    let curr_mip_roughness = ibl_lod_to_roughness(f32(params.read_lod + 1));
    let mip_roughness = roughness_from_relative_mip(prev_mip_roughness, curr_mip_roughness);
    let mip_roughness_clamped = max(mip_roughness, ROUGHNESS_THRESHOLD);
    let cone_cos = cone_cosine_from_roughness(mip_roughness_clamped);
    let out_direction = normalize(ibl_texel_to_direction(vec2<f32>(texel), out_extent));
    let basis = ibl_from_up_axis(out_direction);
    var weight_accum = 0.0;
    var radiance_accum = vec4<f32>(0.0);
    for (var i = 0u; i < CONVOLVE_SAMPLES; i = i + 1u) {
        let rand = ibl_hammersley_2d(i, CONVOLVE_SAMPLES);
        let in_direction = basis * ibl_sample_uniform_cone(rand, cone_cos);
        let in_uv = direction_to_uv(in_direction, f32(params.read_lod), params.mip0_extent);
        let radiance = textureSampleLevel(oct_in, lin_sampler, in_uv, 0.0);
        let w = sample_weight(out_direction, in_direction, mip_roughness_clamped);
        radiance_accum += radiance * w;
        weight_accum += w;
    }
    var out_radiance = vec4<f32>(0.0);
    if (weight_accum != 0.0) {
        out_radiance = radiance_accum / weight_accum;
    }
    textureStore(oct_out, texel, out_radiance);
}

@compute @workgroup_size(8, 8, 1)
fn octahedral_to_cube(@builtin(global_invocation_id) id: vec3<u32>) {
    if (i32(id.x) >= params.face_size || i32(id.y) >= params.face_size) {
        return;
    }
    let dir = cube_texel_direction(id);
    let uv = direction_to_uv(dir, f32(params.read_lod), params.mip0_extent);
    let c = textureSampleLevel(oct_in, lin_sampler, uv, 0.0);
    textureStore(cube_out, vec2<i32>(id.xy), i32(id.z), vec4<f32>(c.rgb, 1.0));
}
