// SPDX-License-Identifier: GPL-3.0-or-later
// EEVEE-like shadow maps (WP 5.1, `eevee/shadows.rs`): depth only, one layer of the
// `Depth32Float` shadow array per pass, reverse depth (1 at the light).

struct Layer {
    view_proj: mat4x4<f32>,
    // x: the sample's transparency random (`SAMPLING_TRANSPARENCY`, WP 5.5), y: the map
    // index.
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
};

@vertex
fn vs_shadow(@location(0) position: vec3<f32>) -> ShadowOut {
    var o: ShadowOut;
    let wp = draw.model * vec4<f32>(position, 1.0);
    o.clip = layer.view_proj * wp;
    return o;
}

// `pcg4d` (eevee_sampling_lib.bsl.hh, Blender 5.2.4, GPL-2.0-or-later), returning 24-bit
// uniform values (exact in f32).
fn shadow_pcg4d(v: vec4<u32>) -> vec4<f32> {
    var u = v * 1664525u + 1013904223u;
    u.x += u.y * u.w;
    u.y += u.z * u.x;
    u.z += u.x * u.y;
    u.w += u.y * u.z;
    u ^= u >> vec4<u32>(16u);
    u.x += u.y * u.w;
    u.y += u.z * u.x;
    u.z += u.x * u.y;
    u.w += u.y * u.z;
    return vec4<f32>(u >> vec4<u32>(8u)) * (1.0 / 16777216.0);
}

// Transparent shadows (`use_transparent_shadow`, `surf_shadow`, WP 5.5): a transparent caster
// discards a fragment when its transparency exceeds a per-sample random threshold; the maps of
// jittered lights are re-rendered every TAA sample, so the samples average the shadow.
// Blender hashes the bits of the interpolated world position (`pcg4d(P, sample random)`); frap
// hashes the shadow-map texel (integral `@builtin(position)`), the map and the sample random
// instead: the interpolated position differs in its last bits between rasterisers, which gave
// every adapter its own noise (deviations D-130). One random per texel, map and sample either
// way.
@fragment
fn fs_shadow(v: ShadowOut) {
    if (draw.flags.w == 1u) {
        let transparency = 1.0 - bitcast<f32>(draw.flags.z);
        let texel = vec2<u32>(v.clip.xy);
        let key = vec4<u32>(texel, u32(layer.params.y), bitcast<u32>(layer.params.x));
        let threshold = shadow_pcg4d(key).x;
        if (transparency > threshold) {
            discard;
        }
    }
}
