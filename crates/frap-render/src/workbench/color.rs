// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench colour sources (`display.shading.color_type`) and Blender's material packing.
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later):
//! - `workbench_materials.cc` `Material::Material(::Object &ob, bool random)`: RANDOM colour is
//!   `hsv_to_rgb(BLI_hash_int_01(hash), 0.5, 0.8)`, where `hash` is
//!   `BLI_ghashutil_strhash_p_murmur(ob->id.name)`: MurmurHash2 (`BLI_hash_mm2`, seed 0) of the
//!   `OB`-prefixed name **including its terminating NUL** (`strlen + 1` bytes), mixed with the
//!   library path for linked objects (verified against Blender renders). `RObject::random_hash`
//!   carries that `hash`; the renderer applies `BLI_hash_int_01` and the HSV conversion.
//!   Non-material colour types use metallic 0 and roughness 0.4; so does a slot without a
//!   material
//!   (`BKE_material_default_empty`, DNA default `roughness = 0.4`).
//! - `Material::pack_data`: roughness is remapped to `sqrt(roughness)` ("Disney roughness"),
//!   then alpha, roughness and metallic are packed in 8 bits each, and the opaque G-buffer
//!   re-packs roughness/metallic in 5/3 bits (`workbench_float_pair_encode`).
//!   The STUDIO lighting therefore sees quantised values; frap applies the same quantisation
//!   on the CPU (verified against Blender renders, `tests/workbench_ref.rs`).
//! - `BLI_hash.h` `BLI_hash_int_2d` / `BLI_hash_int_01`, `math_color.cc` `hsv_to_rgb`.

/// `BLI_hash_int_2d(kx, ky)` (Bob Jenkins' `final` mix).
pub fn bli_hash_int_2d(kx: u32, ky: u32) -> u32 {
    let rot = |x: u32, k: u32| x.rotate_left(k);
    let mut a: u32 = 0xdeadbeef_u32.wrapping_add((2 << 2) + 13);
    let mut b = a;
    let mut c = a;
    b = b.wrapping_add(ky);
    a = a.wrapping_add(kx);
    c ^= b;
    c = c.wrapping_sub(rot(b, 14));
    a ^= c;
    a = a.wrapping_sub(rot(c, 11));
    b ^= a;
    b = b.wrapping_sub(rot(a, 25));
    c ^= b;
    c = c.wrapping_sub(rot(b, 16));
    a ^= c;
    a = a.wrapping_sub(rot(c, 4));
    b ^= a;
    b = b.wrapping_sub(rot(a, 14));
    c ^= b;
    c = c.wrapping_sub(rot(b, 24));
    c
}

/// `BLI_hash_int_01(k)`: `(float)BLI_hash_int(k) * (1.0f / (float)0xFFFFFFFF)`.
pub fn bli_hash_int_01(k: u32) -> f32 {
    bli_hash_int_2d(k, 0) as f32 * (1.0f32 / u32::MAX as f32)
}

/// `hsv_to_rgb` (`BLI_math_color`).
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let nr = ((h * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let ng = (2.0 - (h * 6.0 - 2.0).abs()).clamp(0.0, 1.0);
    let nb = (2.0 - (h * 6.0 - 4.0).abs()).clamp(0.0, 1.0);
    [
        ((nr - 1.0) * s + 1.0) * v,
        ((ng - 1.0) * s + 1.0) * v,
        ((nb - 1.0) * s + 1.0) * v,
    ]
}

/// Workbench RANDOM colour of an object whose name hash is `hash`.
pub fn random_color(hash: u32) -> [f32; 3] {
    hsv_to_rgb(bli_hash_int_01(hash), 0.5, 0.8)
}

/// `unit_float_to_uchar_clamp`.
fn unit_float_to_uchar_clamp(v: f32) -> u32 {
    if v.is_nan() || v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u32
    }
}

/// `(roughness, metallic)` as the STUDIO lighting sees them: `Material::pack_data`
/// (`sqrt(roughness)`, 8 bits each) then the opaque G-buffer's `workbench_float_pair_encode` (5 bits roughness, 3 bits
/// metallic, truncating), decoded.
pub fn gbuffer_roughness_metallic(roughness: f32, metallic: f32) -> (f32, f32) {
    let r8 = unit_float_to_uchar_clamp(roughness.max(0.0).sqrt()) as f32 * (1.0 / 255.0);
    let m8 = unit_float_to_uchar_clamp(metallic) as f32 * (1.0 / 255.0);
    let iv1 = (r8 * 31.0) as i32;
    let iv2 = (m8 * 7.0) as i32;
    (iv1 as f32 * (1.0 / 31.0), iv2 as f32 * (1.0 / 7.0))
}

/// `(roughness, metallic)` as the transparent pass sees them: `Material::pack_data` only
/// (`sqrt(roughness)` and metallic in 8 bits each, `material_data_get`).
pub fn packed_roughness_metallic(roughness: f32, metallic: f32) -> (f32, f32) {
    (
        unit_float_to_uchar_clamp(roughness.max(0.0).sqrt()) as f32 * (1.0 / 255.0),
        unit_float_to_uchar_clamp(metallic) as f32 * (1.0 / 255.0),
    )
}

/// IEEE 754 binary16 bits of `v`, rounded to nearest even (overflow → infinity, NaN kept): the
/// conversion a GPU applies when it stores into a half-float texture.
pub fn f32_to_f16_bits(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let man = x & 0x007f_ffff;
    if exp == 0xff {
        // Inf / NaN.
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        // Subnormal half (or zero).
        if e < -10 {
            return sign;
        }
        let m = man | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rem = m & ((1 << shift) - 1);
        let mid = 1 << (shift - 1);
        let round = rem > mid || (rem == mid && (half & 1) == 1);
        return sign | (half + u32::from(round)) as u16;
    }
    let half = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1fff;
    let round = rem > 0x1000 || (rem == 0x1000 && (half & 1) == 1);
    // A carry out of the mantissa correctly bumps the exponent (up to infinity).
    sign | (half + u32::from(round)) as u16
}

/// The value of binary16 bits.
pub fn f16_bits_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0f32 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let man = f32::from(h & 0x3ff);
    match exp {
        0 => sign * man * (1.0 / 16_777_216.0), // 2^-24
        0x1f if man == 0.0 => sign * f32::INFINITY,
        0x1f => f32::NAN,
        e => sign * (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    }
}

/// `v` rounded to the nearest half float.
pub fn round_f16(v: f32) -> f32 {
    f16_bits_to_f32(f32_to_f16_bits(v))
}

/// `v` stored in, then read from, a half-float render target the way the Apple GPUs that
/// rendered Blender's references do it: rounded toward zero (`workbench.wgsl` `rtz16`).
pub fn round_f16_toward_zero(v: f32) -> f32 {
    let bits = f32_to_f16_bits(v);
    let h = f16_bits_to_f32(bits);
    if h.abs() > v.abs() {
        f16_bits_to_f32(bits - 1)
    } else {
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats() {
        for v in [0.0f32, 1.0, -2.5, 0.1, 65504.0, 6.1e-5, 1e-7, 3.0e-8] {
            let r = round_f16(v);
            assert!((r - v).abs() <= v.abs() * 1e-3 + 6e-8, "{v} -> {r}");
        }
        assert_eq!(f32_to_f16_bits(1.0), 0x3c00);
        assert_eq!(f32_to_f16_bits(-2.0), 0xc000);
        assert_eq!(f32_to_f16_bits(65520.0), 0x7c00);
        assert_eq!(f32_to_f16_bits(0.1), 0x2e66);
        assert_eq!(f16_bits_to_f32(0x3555), 0.333_251_95);
        assert!(round_f16(f32::NAN).is_nan());
        // Ties to even: 1 + 2^-11 is halfway between 1 and 1 + 2^-10.
        assert_eq!(round_f16(1.0 + 1.0 / 2048.0), 1.0);
        // Toward zero: just below 1 truncates to 1 - 2^-11, never up; no overflow to infinity.
        assert_eq!(round_f16_toward_zero(0.99999), 1.0 - 1.0 / 2048.0);
        assert_eq!(round_f16_toward_zero(-0.99999), -(1.0 - 1.0 / 2048.0));
        assert_eq!(round_f16_toward_zero(1.0), 1.0);
        assert_eq!(round_f16_toward_zero(1e6), 65504.0);
    }

    #[test]
    fn hsv_primaries() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), [1.0, 0.0, 0.0]);
        assert_eq!(hsv_to_rgb(1.0 / 3.0, 1.0, 1.0)[1], 1.0);
        assert_eq!(hsv_to_rgb(0.5, 0.0, 0.8), [0.8, 0.8, 0.8]);
    }

    #[test]
    fn hash_is_deterministic_and_spread() {
        assert_eq!(bli_hash_int_2d(0, 0), bli_hash_int_2d(0, 0));
        assert_ne!(bli_hash_int_2d(1, 0), bli_hash_int_2d(2, 0));
        let h = bli_hash_int_01(12345);
        assert!((0.0..=1.0).contains(&h));
    }

    #[test]
    fn packing_quantises_like_blender() {
        // Default roughness 0.4: sqrt -> 0.632 -> 161/255 -> int(0.631 * 31) = 19 -> 19/31.
        let (r, m) = gbuffer_roughness_metallic(0.4, 0.0);
        assert_eq!(r, 19.0 * (1.0 / 31.0));
        assert_eq!(m, 0.0);
        // Metallic 0.7: 179/255 -> int(0.702 * 7) = 4 -> 4/7.
        let (_, m) = gbuffer_roughness_metallic(0.5, 0.7);
        assert_eq!(m, 4.0 * (1.0 / 7.0));
        let (r, m) = gbuffer_roughness_metallic(1.0, 1.0);
        assert_eq!((r, m), (1.0, 1.0));
    }
}
