// SPDX-License-Identifier: GPL-3.0-or-later
// EEVEE-like film accumulation (WP 5.1, `eevee/film.rs`): every film pixel adds the weighted
// render texels of its filter footprint (taps precomputed on the CPU, as EEVEE's
// `Film::update_sample_table`) to the previous accumulator. Rgba32Float ping-pong: read
// accumulator `i`, write `1 - i`.

const MAX_TAPS: u32 = 49u;

struct FilmU {
    // xy: texel offset, z: weight.
    taps: array<vec4<f32>, 49>,
    // x: tap count, yz: render extent.
    count: vec4<u32>,
};

@group(0) @binding(0) var<uniform> film: FilmU;
@group(0) @binding(1) var prev_tx: texture_2d<f32>;
@group(0) @binding(2) var sample_tx: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_accumulate(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy);
    let extent = vec2<i32>(film.count.yz);
    var acc = textureLoad(prev_tx, p, 0);
    let n = min(film.count.x, MAX_TAPS);
    for (var i = 0u; i < n; i++) {
        let t = film.taps[i];
        // Extend on borders, as EEVEE's film does.
        let q = clamp(p + vec2<i32>(t.xy), vec2<i32>(0), extent - 1);
        acc += textureLoad(sample_tx, q, 0) * t.z;
    }
    return acc;
}
