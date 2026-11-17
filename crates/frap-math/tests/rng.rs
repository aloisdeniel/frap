// SPDX-License-Identifier: GPL-3.0-or-later
//! `BliRng` reproduces sequences dumped from Blender 5.2.4 by `tools/blender/dump_rng.py`
//! (WP 1.2 acceptance), and is bit-identical across OSes (pinned bit patterns).

use frap_math::BliRng;
use frap_math::rng::array_randomize;

#[allow(clippy::excessive_precision, clippy::type_complexity)] // generated fixture
mod blender {
    include!("data/blender_rng.rs");
}

/// `object_rand_transverts` (Blender `editors/object/object_random.cc`) with offset = 1,
/// uniform = 1, normal = 0 on vertices at the origin: per vertex one `get_float()` and one
/// `get_unit_float3()`; `loc += vec * (offset * t)` with `t = max(0, 1 + 0 * f) = 1`.
fn vertex_random(seed: u32, n: usize) -> Vec<[f32; 3]> {
    let mut rng = BliRng::new(seed);
    (0..n)
        .map(|_| {
            let (offset, uniform) = (1.0f32, 1.0f32);
            let t = (uniform + ((1.0 - uniform) * rng.get_float())).max(0.0);
            let v = rng.get_unit_float3();
            [
                0.0 + v[0] * (offset * t),
                0.0 + v[1] * (offset * t),
                0.0 + v[2] * (offset * t),
            ]
        })
        .collect()
}

#[test]
fn vertex_random_matches_blender() {
    for (seed, expected) in blender::VERTEX_RANDOM {
        let got = vertex_random(*seed, expected.len());
        for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
            // z = 2 f - 1 involves no transcendental: bit-exact.
            assert_eq!(g[2].to_bits(), e[2].to_bits(), "seed {seed} vertex {i} z");
            // x, y go through cosf/sinf: Blender's platform libm vs `libm`.
            assert!(
                (g[0] - e[0]).abs() <= 1e-6,
                "seed {seed} vertex {i} x {g:?} {e:?}"
            );
            assert!(
                (g[1] - e[1]).abs() <= 1e-6,
                "seed {seed} vertex {i} y {g:?} {e:?}"
            );
        }
    }
}

#[test]
fn select_random_matches_blender() {
    for (seed, ratio, expected) in blender::SELECT_RANDOM {
        let n = 20usize;
        let mut elems: Vec<usize> = (0..n).collect();
        array_randomize(&mut elems, *seed);
        // `const int count_select = elem_map_len * randfac;` (float multiply, truncated).
        let count = (n as f32 * ratio) as usize;
        let mut sel: Vec<usize> = elems[..count].to_vec();
        sel.sort_unstable();
        assert_eq!(&sel[..], *expected, "seed {seed} ratio {ratio}");
    }
}

#[test]
fn sequences_are_bit_identical_across_platforms() {
    // Bit patterns recorded on macOS arm64; CI checks Linux and Windows produce the same.
    let mut r = BliRng::new(1234);
    let floats: Vec<u32> = (0..4).map(|_| r.get_float().to_bits()).collect();
    let mut r = BliRng::new(1234);
    let ints: Vec<u32> = (0..4).map(|_| r.get_uint32()).collect();
    let expect_floats: Vec<u32> = ints
        .iter()
        .map(|i| (*i as i32 as f32 / 2_147_483_648.0).to_bits())
        .collect();
    assert_eq!(floats, expect_floats);
    let mut r = BliRng::new(99);
    let v: Vec<u32> = (0..8)
        .flat_map(|_| r.get_unit_float3())
        .map(f32::to_bits)
        .collect();
    let mut r = BliRng::new(99);
    let w: Vec<u32> = (0..8)
        .flat_map(|_| r.get_unit_float3())
        .map(f32::to_bits)
        .collect();
    assert_eq!(v, w);
    assert_eq!(v, PINNED_UNIT_FLOAT3_SEED_99);
}

/// `BliRng::new(99)`, 8 × `get_unit_float3()`, as f32 bits (recorded with `libm`).
const PINNED_UNIT_FLOAT3_SEED_99: [u32; 24] = PINNED;

include!("data/pinned_rng_bits.rs");
