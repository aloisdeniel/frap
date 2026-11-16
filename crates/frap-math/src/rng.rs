// SPDX-License-Identifier: GPL-3.0-or-later
//! `BliRng`: port of Blender's `blender::RandomNumberGenerator` / `BLI_rng` (drand48-style
//! 48-bit LCG) for `select_random`, `vertex_random` and subdivide fractal (§21.4).
//!
//! Provenance: Blender 5.2.4, `source/blender/blenlib/BLI_rand.hh` and
//! `source/blender/blenlib/intern/rand.cc` (GPL-2.0-or-later), and the first 256 entries of
//! `BLI_noise_hash_uchar_512` from `blenlib/intern/noise_c.cc` (the table repeats itself after
//! 256; `seed_random` only indexes `seed & 255`). Verified against sequences dumped from Blender
//! by `tools/blender/dump_rng.py` (`crates/frap-math/tests/rng.rs`).
//!
//! Every transcendental goes through `libm` (rule 2.2.12), so the sequences are bit-identical
//! on every OS.

const MULTIPLIER: u64 = 0x5_DEEC_E66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const LOWSEED: u64 = 0x330E;
/// `float(M_PI * 2.0)`
const TWO_PI_F32: f32 = (std::f64::consts::PI * 2.0) as f32;

/// `BLI_noise_hash_uchar_512[0..256]`.
const HASH: [u8; 256] = [
    0xA2, 0xA0, 0x19, 0x3B, 0xF8, 0xEB, 0xAA, 0xEE, 0xF3, 0x1C, 0x67, 0x28, 0x1D, 0xED, 0x00, 0xDE,
    0x95, 0x2E, 0xDC, 0x3F, 0x3A, 0x82, 0x35, 0x4D, 0x6C, 0xBA, 0x36, 0xD0, 0xF6, 0x0C, 0x79, 0x32,
    0xD1, 0x59, 0xF4, 0x08, 0x8B, 0x63, 0x89, 0x2F, 0xB8, 0xB4, 0x97, 0x83, 0xF2, 0x8F, 0x18, 0xC7,
    0x51, 0x14, 0x65, 0x87, 0x48, 0x20, 0x42, 0xA8, 0x80, 0xB5, 0x40, 0x13, 0xB2, 0x22, 0x7E, 0x57,
    0xBC, 0x7F, 0x6B, 0x9D, 0x86, 0x4C, 0xC8, 0xDB, 0x7C, 0xD5, 0x25, 0x4E, 0x5A, 0x55, 0x74, 0x50,
    0xCD, 0xB3, 0x7A, 0xBB, 0xC3, 0xCB, 0xB6, 0xE2, 0xE4, 0xEC, 0xFD, 0x98, 0x0B, 0x96, 0xD3, 0x9E,
    0x5C, 0xA1, 0x64, 0xF1, 0x81, 0x61, 0xE1, 0xC4, 0x24, 0x72, 0x49, 0x8C, 0x90, 0x4B, 0x84, 0x34,
    0x38, 0xAB, 0x78, 0xCA, 0x1F, 0x01, 0xD7, 0x93, 0x11, 0xC1, 0x58, 0xA9, 0x31, 0xF9, 0x44, 0x6D,
    0xBF, 0x33, 0x9C, 0x5F, 0x09, 0x94, 0xA3, 0x85, 0x06, 0xC6, 0x9A, 0x1E, 0x7B, 0x46, 0x15, 0x30,
    0x27, 0x2B, 0x1B, 0x71, 0x3C, 0x5B, 0xD6, 0x6F, 0x62, 0xAC, 0x4F, 0xC2, 0xC0, 0x0E, 0xB1, 0x23,
    0xA7, 0xDF, 0x47, 0xB0, 0x77, 0x69, 0x05, 0xE9, 0xE6, 0xE7, 0x76, 0x73, 0x0F, 0xFE, 0x6E, 0x9B,
    0x56, 0xEF, 0x12, 0xA5, 0x37, 0xFC, 0xAE, 0xD9, 0x03, 0x8E, 0xDD, 0x10, 0xB9, 0xCE, 0xC9, 0x8D,
    0xDA, 0x2A, 0xBD, 0x68, 0x17, 0x9F, 0xBE, 0xD4, 0x0A, 0xCC, 0xD2, 0xE8, 0x43, 0x3D, 0x70, 0xB7,
    0x02, 0x7D, 0x99, 0xD8, 0x0D, 0x60, 0x8A, 0x04, 0x2C, 0x3E, 0x92, 0xE5, 0xAF, 0x53, 0x07, 0xE0,
    0x29, 0xA6, 0xC5, 0xE3, 0xF5, 0xF7, 0x4A, 0x41, 0x26, 0x6A, 0x16, 0x5E, 0x52, 0x2D, 0x21, 0xAD,
    0xF0, 0x91, 0xFF, 0xEA, 0x54, 0xFA, 0x66, 0x1A, 0x45, 0x39, 0xCF, 0x75, 0xA4, 0x88, 0xFB, 0x5D,
];

/// Blender's `RNG` (48-bit LCG).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BliRng {
    /// 48-bit state.
    pub x: u64,
}

impl Default for BliRng {
    /// `RandomNumberGenerator(0)`.
    fn default() -> Self {
        BliRng::new(0)
    }
}

impl BliRng {
    /// `BLI_rng_new(seed)` / `RandomNumberGenerator rng(seed)`.
    pub fn new(seed: u32) -> BliRng {
        let mut r = BliRng { x: 0 };
        r.seed(seed);
        r
    }

    /// `BLI_rng_new_srandom(seed)`: seeded with a hash of `seed`.
    pub fn new_srandom(seed: u32) -> BliRng {
        let mut r = BliRng { x: 0 };
        r.seed_random(seed);
        r
    }

    /// `seed(seed)`: `x = (seed << 16) | 0x330E`.
    pub fn seed(&mut self, seed: u32) {
        self.x = (u64::from(seed) << 16) | LOWSEED;
    }

    /// `seed_random(seed)`: three rounds of `seed + hash[seed & 255]`.
    pub fn seed_random(&mut self, seed: u32) {
        let mut seed = seed;
        self.seed(seed.wrapping_add(u32::from(HASH[(seed & 255) as usize])));
        seed = self.get_uint32();
        self.seed(seed.wrapping_add(u32::from(HASH[(seed & 255) as usize])));
        seed = self.get_uint32();
        self.seed(seed.wrapping_add(u32::from(HASH[(seed & 255) as usize])));
    }

    fn step(&mut self) {
        self.x = (MULTIPLIER.wrapping_mul(self.x).wrapping_add(ADDEND)) & MASK;
    }

    /// `get_uint32()` (`BLI_rng_get_uint`): the top 31 bits of the next state.
    pub fn get_uint32(&mut self) -> u32 {
        self.step();
        (self.x >> 17) as u32
    }

    /// `get_int32()` (`BLI_rng_get_int`); never negative (31 bits).
    pub fn get_int32(&mut self) -> i32 {
        self.step();
        (self.x >> 17) as i32
    }

    /// `get_uint64()`: high word first.
    pub fn get_uint64(&mut self) -> u64 {
        let hi = u64::from(self.get_uint32());
        let lo = u64::from(self.get_uint32());
        (hi << 32) | lo
    }

    /// `get_int32(max_exclusive)`: `get_int32() % max_exclusive` (0..max, never max).
    pub fn get_int32_below(&mut self, max_exclusive: i32) -> i32 {
        debug_assert!(max_exclusive > 0);
        self.get_int32() % max_exclusive
    }

    /// `get_double()`: [0, 1).
    pub fn get_double(&mut self) -> f64 {
        f64::from(self.get_int32()) / 2_147_483_648.0
    }

    /// `get_float()` (`BLI_rng_get_float`): [0, 1).
    pub fn get_float(&mut self) -> f32 {
        self.get_int32() as f32 / 2_147_483_648.0
    }

    /// `skip(n)`: simulates `n` draws.
    pub fn skip(&mut self, n: u64) {
        for _ in 0..n {
            self.step();
        }
    }

    /// `get_unit_float2()`: a random unit 2D vector.
    pub fn get_unit_float2(&mut self) -> [f32; 2] {
        let a = TWO_PI_F32 * self.get_float();
        [libm::cosf(a), libm::sinf(a)]
    }

    /// `get_unit_float3()` (`BLI_rng_get_float_unit_v3`): a random unit 3D vector.
    pub fn get_unit_float3(&mut self) -> [f32; 3] {
        let z = (2.0 * self.get_float()) - 1.0;
        let r = 1.0 - z * z;
        if r > 0.0 {
            let a = TWO_PI_F32 * self.get_float();
            let r = r.sqrt();
            let x = r * libm::cosf(a);
            let y = r * libm::sinf(a);
            return [x, y, z];
        }
        [0.0, 0.0, 1.0]
    }

    /// `get_barycentric_coordinates()`.
    pub fn get_barycentric_coordinates(&mut self) -> [f32; 3] {
        let (r1, r2) = self.triangle_uv();
        [r1, r2, 1.0 - r1 - r2]
    }

    /// `round_probabilistic(x)` (`x >= 0`): 4.9 rounds to 5 more often than 4.6 does.
    pub fn round_probabilistic(&mut self, x: f32) -> i32 {
        debug_assert!(x >= 0.0);
        let round_up_probability = x - x.floor();
        let round_up = round_up_probability > self.get_float();
        x as i32 + i32::from(round_up)
    }

    /// `get_triangle_sample(v1, v2, v3)` (`BLI_rng_get_tri_sample_float_v2`).
    pub fn get_triangle_sample(&mut self, v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> [f32; 2] {
        let (u, v) = self.triangle_uv();
        let mut s = v1;
        for k in 0..2 {
            s[k] += (v2[k] - v1[k]) * u;
        }
        for k in 0..2 {
            s[k] += (v3[k] - v1[k]) * v;
        }
        s
    }

    /// `get_triangle_sample_3d(v1, v2, v3)`.
    pub fn get_triangle_sample_3d(&mut self, v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> [f32; 3] {
        let (u, v) = self.triangle_uv();
        let mut s = v1;
        for k in 0..3 {
            s[k] += (v2[k] - v1[k]) * u;
        }
        for k in 0..3 {
            s[k] += (v3[k] - v1[k]) * v;
        }
        s
    }

    fn triangle_uv(&mut self) -> (f32, f32) {
        let mut u = self.get_float();
        let mut v = self.get_float();
        if u + v > 1.0 {
            u = 1.0 - u;
            v = 1.0 - v;
        }
        (u, v)
    }

    /// `get_bytes(r_bytes)` (`BLI_rng_get_char_n`): the 6 low bytes of the state in
    /// little-endian order per step (what Blender produces on every platform it supports).
    pub fn get_bytes(&mut self, out: &mut [u8]) {
        const STRIDE: usize = 6;
        let n = out.len();
        let (trim_len, last_len) = if n > STRIDE {
            (n - n % STRIDE, n % STRIDE)
        } else {
            (0, n)
        };
        let mut i = 0;
        while i != trim_len {
            let src = self.x.to_le_bytes();
            out[i..i + STRIDE].copy_from_slice(&src[..STRIDE]);
            i += STRIDE;
            self.step();
        }
        if last_len > 0 {
            let src = self.x.to_le_bytes();
            out[i..i + last_len].copy_from_slice(&src[..last_len]);
        }
    }

    /// `BLI_rng_shuffle_array`: from the last element down, `j = get_uint32() % n`.
    pub fn shuffle_array<T>(&mut self, data: &mut [T]) {
        let n = data.len();
        if n <= 1 {
            return;
        }
        let num = n as u32;
        let mut i = n;
        while i > 0 {
            i -= 1;
            let j = (self.get_uint32() % num) as usize;
            if i != j {
                data.swap(i, j);
            }
        }
    }

    /// `RandomNumberGenerator::shuffle` (the C++ template used by newer code): from the last
    /// element down to index 2, `j = get_int32(i)`.
    pub fn shuffle<T>(&mut self, values: &mut [T]) {
        debug_assert!(values.len() <= i32::MAX as usize);
        let mut i = values.len() as i64 - 1;
        while i >= 2 {
            let j = i64::from(self.get_int32_below(i as i32));
            if i != j {
                values.swap(i as usize, j as usize);
            }
            i -= 1;
        }
    }
}

/// `BLI_array_randomize(data, ..., seed)`: shuffle with a plainly seeded generator
/// (`mesh.select_random`).
pub fn array_randomize<T>(data: &mut [T], seed: u32) {
    BliRng::new(seed).shuffle_array(data);
}

/// `BLI_hash_frand(seed)`: one float from a hash-seeded generator.
pub fn hash_frand(seed: u32) -> f32 {
    BliRng::new_srandom(seed).get_float()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_layout_matches_blender() {
        assert_eq!(BliRng::new(0).x, 0x330E);
        assert_eq!(BliRng::new(1).x, 0x1_330E);
        assert_eq!(BliRng::new(u32::MAX).x, 0xFFFF_FFFF_330E);
        assert_eq!(BliRng::default(), BliRng::new(0));
    }

    #[test]
    fn drand48_reference_sequence() {
        // Same LCG and initial state as POSIX `srand48(0)`; values are `lrand48()`'s.
        let mut r = BliRng::new(0);
        let got: Vec<u32> = (0..4).map(|_| r.get_uint32()).collect();
        assert_eq!(
            got,
            [366_850_414, 1_610_402_240, 206_956_554, 1_869_309_841]
        );
        let mut r = BliRng::new(0);
        r.skip(3);
        assert_eq!(r.get_uint32(), 1_869_309_841);
        let mut r = BliRng::new(0);
        assert_eq!(r.get_uint64(), (366_850_414_u64 << 32) | 1_610_402_240);
    }

    #[test]
    fn floats_in_unit_interval_and_bytes_consistent() {
        let mut r = BliRng::new(42);
        for _ in 0..1000 {
            let f = r.get_float();
            assert!((0.0..1.0).contains(&f));
            let d = r.get_double();
            assert!((0.0..1.0).contains(&d));
            let i = r.get_int32_below(7);
            assert!((0..7).contains(&i));
        }
        let mut a = BliRng::new(7);
        let mut b = BliRng::new(7);
        let mut buf = [0u8; 13];
        a.get_bytes(&mut buf);
        assert_eq!(buf[..6], b.x.to_le_bytes()[..6]);
        b.step();
        assert_eq!(buf[6..12], b.x.to_le_bytes()[..6]);
        b.step();
        assert_eq!(buf[12], b.x.to_le_bytes()[0]);
        assert_eq!(a, b);
    }

    #[test]
    fn samples_are_well_formed() {
        let mut r = BliRng::new(11);
        for _ in 0..200 {
            let [x, y, z] = r.get_unit_float3();
            assert!(((x * x + y * y + z * z) - 1.0).abs() < 1e-5);
            let [a, b] = r.get_unit_float2();
            assert!(((a * a + b * b) - 1.0).abs() < 1e-5);
            let w = r.get_barycentric_coordinates();
            assert!(w.iter().all(|c| (-1e-6..=1.0).contains(c)));
            let p = r.get_triangle_sample([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]);
            assert!(p[0] + p[1] <= 1.0 + 1e-6);
            let q = r.get_triangle_sample_3d([0.0; 3], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]);
            assert!(q[0] + q[1] <= 2.0 + 1e-5 && q[2] == 0.0);
            let n = r.round_probabilistic(4.25);
            assert!(n == 4 || n == 5);
        }
        assert_eq!(r.round_probabilistic(3.0), 3);
    }

    #[test]
    fn shuffles_are_permutations() {
        let mut v: Vec<u32> = (0..50).collect();
        array_randomize(&mut v, 3);
        let mut s = v.clone();
        s.sort_unstable();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
        assert_ne!(v, s);
        let mut w: Vec<u32> = (0..50).collect();
        BliRng::new(3).shuffle(&mut w);
        let mut t = w.clone();
        t.sort_unstable();
        assert_eq!(t, s);
        let mut one = [1];
        array_randomize(&mut one, 9);
        assert_eq!(one, [1]);
    }

    #[test]
    fn srandom_differs_from_seed_and_is_deterministic() {
        assert_ne!(BliRng::new_srandom(5), BliRng::new(5));
        assert_eq!(BliRng::new_srandom(5), BliRng::new_srandom(5));
        assert_eq!(hash_frand(9).to_bits(), hash_frand(9).to_bits());
        // First round by hand: seed 0 -> seed(0 + hash[0] = 0xA2).
        let mut r = BliRng::new(0xA2);
        let s1 = r.get_uint32();
        let mut r = BliRng::new(s1.wrapping_add(u32::from(HASH[(s1 & 255) as usize])));
        let s2 = r.get_uint32();
        let expect = BliRng::new(s2.wrapping_add(u32::from(HASH[(s2 & 255) as usize])));
        assert_eq!(BliRng::new_srandom(0), expect);
    }
}
