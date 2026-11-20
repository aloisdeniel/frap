// SPDX-License-Identifier: GPL-3.0-or-later
// IBL preview (WP 5.2 tests only; prepend common.wgsl and shading.wgsl): ray-casts analytic
// spheres and horizontal ground quads lit by a `WorldLighting` (L1 diffuse world, prefiltered
// probe, world sun with hard ray-traced shadows, background), with the film's
// Gaussian-fitted Blackman-Harris filter (`film_filter_weight`, radius `filter_size` 1.5 px)
// over a 6 x 6 sample grid. It validates the world lighting against Blender's EEVEE renders
// before the forward engine (WP 5.1) is wired to it, and consumes every `WorldLighting` field
// exactly as its documentation tells consumers to (the contract test of the goldens).

struct Uniforms {
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_back: vec4<f32>,
    cam_pos: vec4<f32>,
    // tan of the half field of view (x, y), image size (z, w).
    frustum: vec4<f32>,
    // `WorldLighting::sh9`.
    sh: array<vec4<f32>, 9>,
    // Direction to the sun (xyz), 1 when there is a sun (w).
    sun_dir: vec4<f32>,
    // Sun irradiance (rgb).
    sun_color: vec4<f32>,
    // rotation, spheres, quads, strength
    misc: vec4<f32>,
}

struct Shape {
    // Sphere: centre, radius. Quad (normal +Z): centre, half size.
    geo: vec4<f32>,
    // Base colour, metallic.
    mat0: vec4<f32>,
    // Roughness, IOR.
    mat1: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> spheres: array<Shape>;
@group(0) @binding(2) var<storage, read> quads: array<Shape>;
@group(0) @binding(3) var specular_tx: texture_cube<f32>;
@group(0) @binding(4) var brdf_tx: texture_2d<f32>;
@group(0) @binding(5) var bsdf_tx: texture_2d_array<f32>;
@group(0) @binding(6) var background_tx: texture_2d<f32>;
@group(0) @binding(7) var lut_sampler: sampler;
@group(0) @binding(8) var bg_sampler: sampler;
@group(0) @binding(9) var out_tx: texture_storage_2d<rgba32float, write>;

struct Hit {
    t: f32,
    n: vec3<f32>,
    mat0: vec4<f32>,
    mat1: vec4<f32>,
}

fn trace(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> Hit {
    var hit = Hit(t_max, vec3<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    let ns = u32(u.misc.y);
    for (var i = 0u; i < ns; i = i + 1u) {
        let s = spheres[i];
        let oc = o - s.geo.xyz;
        let b = dot(oc, d);
        let c = dot(oc, oc) - s.geo.w * s.geo.w;
        let disc = b * b - c;
        if (disc < 0.0) {
            continue;
        }
        let sq = sqrt(disc);
        var t = -b - sq;
        if (t < 1e-4) {
            t = -b + sq;
        }
        if (t > 1e-4 && t < hit.t) {
            hit = Hit(t, normalize(oc + d * t), s.mat0, s.mat1);
        }
    }
    let nq = u32(u.misc.z);
    for (var i = 0u; i < nq; i = i + 1u) {
        let q = quads[i];
        if (abs(d.z) < 1e-8) {
            continue;
        }
        let t = (q.geo.z - o.z) / d.z;
        if (t > 1e-4 && t < hit.t) {
            let p = o + d * t;
            if (abs(p.x - q.geo.x) <= q.geo.w && abs(p.y - q.geo.y) <= q.geo.w) {
                // Two-sided, facing the ray.
                hit = Hit(t, vec3<f32>(0.0, 0.0, select(-1.0, 1.0, d.z < 0.0)), q.mat0, q.mat1);
            }
        }
    }
    return hit;
}

fn ggx_d(nh: f32, a2: f32) -> f32 {
    let t = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (IBL_PI * t * t);
}

fn smith_vis(nl: f32, nv: f32, a2: f32) -> f32 {
    let gl = nv * sqrt(nl * nl * (1.0 - a2) + a2);
    let gv = nl * sqrt(nv * nv * (1.0 - a2) + a2);
    return 0.5 / max(gl + gv, 1e-8);
}

fn shade(o: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let hit = trace(o, d, 1e30);
    if (hit.t >= 1e30) {
        // `WorldLighting::background`: the environment at strength 1.
        let tc = ibl_equirect_tex_coord(d, u.misc.x);
        return textureSampleLevel(background_tx, bg_sampler, tc, 0.0).rgb * u.misc.w;
    }
    let p = o + d * hit.t;
    let n = hit.n;
    let v = -d;
    let nv = dot(n, v);
    let cl = ibl_principled(brdf_tx, bsdf_tx, lut_sampler, hit.mat0.rgb, hit.mat0.w, hit.mat1.x,
                            hit.mat1.y, nv);
    var c = ibl_world_radiance(specular_tx, lut_sampler, u.sh, u.misc.x, u.misc.w, cl, n, v);
    if (u.sun_dir.w > 0.0) {
        let l = u.sun_dir.xyz;
        let nl = dot(n, l);
        if (nl > 0.0) {
            let shadow = trace(p + n * 1e-3, l, 1e30);
            if (shadow.t >= 1e30) {
                let e = u.sun_color.rgb;
                c += cl.diffuse_color * e * nl / IBL_PI;
                // GGX, normalised by the split-sum directional albedo (the closure colour
                // carries the Fresnel), as EEVEE's LTC light evaluation is.
                let a = max(cl.roughness * cl.roughness, 2e-3);
                let a2 = a * a;
                let h = normalize(l + v);
                let lut = ibl_brdf_lut(brdf_tx, lut_sampler, max(nv, 1e-4), cl.roughness);
                let spec = ggx_d(max(dot(n, h), 0.0), a2) * smith_vis(nl, max(nv, 1e-4), a2) * nl;
                c += cl.reflection_color * e * spec / max(lut.x + lut.y, 1e-4);
            }
        }
    }
    return c;
}

const GRID: i32 = 6;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<u32>(u32(u.frustum.z), u32(u.frustum.w));
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    // `film_filter_weight`: Gaussian fitted to Blackman-Harris, sigma 0.284 * filter_size.
    let radius = 1.5;
    var acc = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var j = 0; j < GRID; j = j + 1) {
        for (var i = 0; i < GRID; i = i + 1) {
            let off = (vec2<f32>(f32(i), f32(j)) + 0.5) / f32(GRID) * 2.0 * radius - radius;
            let r2 = dot(off, off) / (radius * radius);
            if (r2 > 1.0) {
                continue;
            }
            let w = exp(-0.5 / (0.284 * 0.284) * r2);
            let px = vec2<f32>(id.xy) + 0.5 + off;
            let ndc = vec2<f32>(px.x / f32(size.x) * 2.0 - 1.0, 1.0 - px.y / f32(size.y) * 2.0);
            let dir = normalize(u.cam_right.xyz * ndc.x * u.frustum.x
                                + u.cam_up.xyz * ndc.y * u.frustum.y - u.cam_back.xyz);
            acc += shade(u.cam_pos.xyz, dir) * w;
            wsum += w;
        }
    }
    textureStore(out_tx, vec2<i32>(id.xy), vec4<f32>(acc / wsum, 1.0));
}
