// SPDX-License-Identifier: GPL-3.0-or-later
// EEVEE-like shadow maps (WP 5.1, `eevee/shadows.rs`): depth only, one layer of the
// `Depth32Float` shadow array per pass, reverse depth (1 at the light).

struct Layer {
    view_proj: mat4x4<f32>,
    // x: the sample's transparency random (`SAMPLING_TRANSPARENCY`, WP 5.5).
    params: vec4<f32>,
};

struct Draw {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    // z: the caster's alpha (f32 bits) when w is 1 (a transparent material, WP 5.5).
    flags: vec4<u32>,
};

@group(0) @binding(0) var<uniform> layer: Layer;
@group(1) @binding(0) var<uniform> draw: Draw;

struct ShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
};

@vertex
fn vs_shadow(@location(0) position: vec3<f32>) -> ShadowOut {
    var o: ShadowOut;
    let wp = draw.model * vec4<f32>(position, 1.0);
    o.clip = layer.view_proj * wp;
    o.world_pos = wp.xyz;
    return o;
}

// `pcg4d` (eevee_sampling_lib.bsl.hh, Blender 5.2.4, GPL-2.0-or-later).
fn shadow_pcg4d(v: vec4<f32>) -> vec4<f32> {
    var u = bitcast<vec4<u32>>(v);
    u = u * 1664525u + 1013904223u;
    u.x += u.y * u.w;
    u.y += u.z * u.x;
    u.z += u.x * u.y;
    u.w += u.y * u.z;
    u ^= u >> vec4<u32>(16u);
    u.x += u.y * u.w;
    u.y += u.z * u.x;
    u.z += u.x * u.y;
    u.w += u.y * u.z;
    return vec4<f32>(u) / f32(0xffffffffu);
}

// Transparent shadows (`use_transparent_shadow`, `surf_shadow`, WP 5.5): a transparent caster
// discards a fragment when its transparency exceeds `pcg4d(P, sample random)`; the maps of
// jittered lights are re-rendered every TAA sample, so the samples average the shadow.
@fragment
fn fs_shadow(v: ShadowOut) {
    if (draw.flags.w == 1u) {
        let transparency = 1.0 - bitcast<f32>(draw.flags.z);
        let threshold = shadow_pcg4d(vec4<f32>(v.world_pos, layer.params.x)).x;
        if (transparency > threshold) {
            discard;
        }
    }
}
