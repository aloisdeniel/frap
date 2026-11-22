// SPDX-License-Identifier: GPL-3.0-or-later
// EEVEE-like shadow maps (WP 5.1, `eevee/shadows.rs`): depth only, one layer of the
// `Depth32Float` shadow array per pass, reverse depth (1 at the light).
//
// The fragment writes its own depth: the clip-space z and w interpolated as varyings
// (perspective-correct, so z / w is the exact depth of the surface point) minus the slope
// bias `SLOPE_BIAS × max(|∂depth/∂x|, |∂depth/∂y|)` (D3D's `SlopeScaledDepthBias` formula).
// The rasterised depth is not precise enough: a caster that passes behind the light (a ground
// plane under a point light) is clipped at the near plane, where the depth is 1, while the
// depths around it are ~1e-4 (`clip_near = influence / 4000`); WARP interpolated such
// triangles' depth with errors of ±4e-5 (5% of the depth), shadowing or lighting whole cube
// faces, and its hardware slope bias followed the same error (ARCHITECTURE Decision log,
// M56CI-1).

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

// Reverse depth: push the occluders away from the light (smaller depth) by this many depth
// slopes, as the former `DepthBiasState::slope_scale` of -1.5 did.
const SLOPE_BIAS: f32 = 1.5;

struct ShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) zw: vec2<f32>,
};

@vertex
fn vs_shadow(@location(0) position: vec3<f32>) -> ShadowOut {
    var o: ShadowOut;
    let wp = draw.model * vec4<f32>(position, 1.0);
    o.clip = layer.view_proj * wp;
    o.zw = o.clip.zw;
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
// every adapter its own noise (deviations D-150). One random per texel, map and sample either
// way.
@fragment
fn fs_shadow(v: ShadowOut) -> @builtin(frag_depth) f32 {
    // Derivatives first, in uniform control flow.
    let depth = v.zw.x / v.zw.y;
    let slope = max(abs(dpdx(depth)), abs(dpdy(depth)));
    if (draw.flags.w == 1u) {
        let transparency = 1.0 - bitcast<f32>(draw.flags.z);
        let texel = vec2<u32>(v.clip.xy);
        let key = vec4<u32>(texel, u32(layer.params.y), bitcast<u32>(layer.params.x));
        let threshold = shadow_pcg4d(key).x;
        if (transparency > threshold) {
            discard;
        }
    }
    return clamp(depth - SLOPE_BIAS * slope, 0.0, 1.0);
}
