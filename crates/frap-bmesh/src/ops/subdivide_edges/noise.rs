// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_noise_generic_turbulence` with the "new Perlin" basis (`noisebasis = 2`), the only
//! noise the subdivide fractal uses.
//!
//! Ported from blender/source/blender/blenlib/intern/noise_c.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `newPerlin`, `newPerlinU`, `npfade`, `grad`, `lerp` and
//! `BLI_noise_generic_turbulence`. `BLI_noise_hash_uchar_512` is its first 256 entries twice
//! (see `frap_math::rng`).

/// `BLI_noise_hash_uchar_512[0..256]` (the table repeats itself after 256).
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

/// `hash[i]` for `i < 512`.
#[inline]
fn hash(i: i32) -> i32 {
    i32::from(HASH[(i & 255) as usize])
}

#[inline]
fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

#[inline]
fn npfade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn grad(hash_val: i32, x: f32, y: f32, z: f32) -> f32 {
    let h = hash_val & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if (h & 1) == 0 { u } else { -u }) + (if (h & 2) == 0 { v } else { -v })
}

/// `newPerlin`.
fn new_perlin(mut x: f32, mut y: f32, mut z: f32) -> f32 {
    let u = libm::floorf(x);
    let v = libm::floorf(y);
    let w = libm::floorf(z);
    let xi = (u as i32) & 255;
    let yi = (v as i32) & 255;
    let zi = (w as i32) & 255;
    x -= u;
    y -= v;
    z -= w;
    let u = npfade(x);
    let v = npfade(y);
    let w = npfade(z);
    let a = hash(xi) + yi;
    let aa = hash(a) + zi;
    let ab = hash(a + 1) + zi;
    let b = hash(xi + 1) + yi;
    let ba = hash(b) + zi;
    let bb = hash(b + 1) + zi;
    lerp(
        w,
        lerp(
            v,
            lerp(u, grad(hash(aa), x, y, z), grad(hash(ba), x - 1.0, y, z)),
            lerp(
                u,
                grad(hash(ab), x, y - 1.0, z),
                grad(hash(bb), x - 1.0, y - 1.0, z),
            ),
        ),
        lerp(
            v,
            lerp(
                u,
                grad(hash(aa + 1), x, y, z - 1.0),
                grad(hash(ba + 1), x - 1.0, y, z - 1.0),
            ),
            lerp(
                u,
                grad(hash(ab + 1), x, y - 1.0, z - 1.0),
                grad(hash(bb + 1), x - 1.0, y - 1.0, z - 1.0),
            ),
        ),
    )
}

/// `newPerlinU`.
fn new_perlin_u(x: f32, y: f32, z: f32) -> f32 {
    0.5 + 0.5 * new_perlin(x, y, z)
}

/// `BLI_noise_generic_turbulence(noisesize, x, y, z, oct, hard, 2)`.
pub(crate) fn turbulence_new_perlin(
    mut noisesize: f32,
    mut x: f32,
    mut y: f32,
    mut z: f32,
    oct: i32,
    hard: bool,
) -> f32 {
    if noisesize != 0.0 {
        noisesize = 1.0 / noisesize;
        x *= noisesize;
        y *= noisesize;
        z *= noisesize;
    }
    let mut sum = 0.0f32;
    let mut amp = 1.0f32;
    let mut fscale = 1.0f32;
    for _ in 0..=oct {
        let mut t = new_perlin_u(fscale * x, fscale * y, fscale * z);
        if hard {
            t = (2.0 * t - 1.0).abs();
        }
        sum += t * amp;
        amp *= 0.5;
        fscale *= 2.0;
    }
    sum *= ((1i32 << oct) as f32) / (((1i32 << (oct + 1)) - 1) as f32);
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perlin_is_zero_on_lattice_points() {
        assert_eq!(new_perlin(0.0, 0.0, 0.0), 0.0);
        assert_eq!(new_perlin(3.0, -2.0, 7.0), 0.0);
    }

    #[test]
    fn turbulence_stays_in_unit_range() {
        for i in 0..50 {
            let f = i as f32 * 0.37;
            let t = turbulence_new_perlin(1.0, f, f * 1.3, -f, 15, false);
            assert!((0.0..=1.0).contains(&t), "{t}");
        }
    }
}
