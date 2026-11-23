// SPDX-License-Identifier: GPL-3.0-or-later
// EEVEE-like shadow maps (WP 5.1, `eevee/shadows.rs`): depth only, one layer of the
// `Depth32Float` shadow array per pass, reverse depth (1 at the light).
//
// The fragment writes its own depth: the depth of the point where the ray of its texel centre
// meets the plane of the triangle, minus the slope bias `SLOPE_BIAS × max(|∂depth/∂x|,
// |∂depth/∂y|)` (D3D's `SlopeScaledDepthBias` formula). Neither the rasterised depth nor any
// interpolated varying can be trusted for a caster that passes behind the light (a ground
// plane under a point light): the clipper puts the triangle's new vertices on the near plane,
// at depth 1 and 1 / w = 1 / clip_near (`clip_near = influence / 4000`), hundreds of
// thousands of texels off the map, and WARP's screen-space interpolation from such vertices is
// off by up to ~1e-4 in depth (most of the ~1e-4 depth around such a light) on the +X face:
// whole cube faces were shadowed or lit (ARCHITECTURE Decision log, M56CI-1 and M56CI-7). The
// perspective-correct varyings carry the same error (the interpolated z / w of M56CI-1 did),
// but every interpolated world position still lies in the triangle's plane, so the plane
// through it spanned by its screen derivatives is exact, and so is the texel's ray (its centre
// and the map's inverse matrix): their intersection does not depend on the interpolation.

struct Layer {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // xy: the map's texel origin in its atlas layer (its viewport), z: its size in texels.
    viewport: vec4<f32>,
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
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_shadow(@location(0) position: vec3<f32>) -> ShadowOut {
    var o: ShadowOut;
    let wp = draw.model * vec4<f32>(position, 1.0);
    o.clip = layer.view_proj * wp;
    o.zw = o.clip.zw;
    o.world = wp.xyz;
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

// The depth of the caster's surface at texel centre `frag` (framebuffer coordinates):
// the intersection of the texel's ray with the plane through `P` spanned by `dPdx` and
// `dPdy`; `fallback` (the interpolated depth) when the plane is degenerate or seen edge-on.
fn plane_depth(frag: vec2<f32>, P: vec3<f32>, dPdx: vec3<f32>, dPdy: vec3<f32>,
               fallback: f32) -> f32 {
    let uv = (frag - layer.viewport.xy) / layer.viewport.z;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    // Reverse depth: 1 at the near plane, 0 at the far plane (both finite).
    let a = layer.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let b = layer.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let A = a.xyz / a.w;
    let dir = b.xyz / b.w - A;
    let n = cross(dPdx, dPdy);
    let den = dot(n, dir);
    // Edge-on (or degenerate) planes: |cos| below 1e-4 between the normal and the ray.
    // Branch-free and without a division by zero (FXC may assume no NaN).
    let edge_on = abs(den) <= 1e-4 * length(n) * length(dir);
    let X = A + dir * (dot(n, P - A) / select(den, 1.0, edge_on));
    let c = layer.view_proj * vec4<f32>(X, 1.0);
    let depth = c.z / select(c.w, 1.0, c.w <= 0.0);
    // Behind the light or outside the depth range: the interpolated depth.
    return select(depth, fallback, edge_on || c.w <= 0.0 || abs(depth - 0.5) > 1.0);
}

// Transparent shadows (`use_transparent_shadow`, `surf_shadow`, WP 5.5): a transparent caster
// discards a fragment when its transparency exceeds a per-sample random threshold; the maps of
// jittered lights are re-rendered every TAA sample, so the samples average the shadow.
// Blender hashes the bits of the interpolated world position (`pcg4d(P, sample random)`); frap
// hashes the shadow-map texel (integral `@builtin(position)`), the map and the sample random
// instead: the interpolated position differs in its last bits between rasterisers, which gave
// every adapter its own noise (deviations D-170). One random per texel, map and sample either
// way.
@fragment
fn fs_shadow(v: ShadowOut) -> @builtin(frag_depth) f32 {
    // Derivatives first, in uniform control flow.
    let dPdx = dpdx(v.world);
    let dPdy = dpdy(v.world);
    let interpolated = v.zw.x / v.zw.y;
    let depth = plane_depth(v.clip.xy, v.world, dPdx, dPdy, interpolated);
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
