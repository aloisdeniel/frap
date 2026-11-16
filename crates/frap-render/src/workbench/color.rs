// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench colour sources (`display.shading.color_type`) and Blender's material packing.
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later):
//! - `workbench_materials.cc` `Material::Material(::Object &ob, bool random)`: RANDOM colour is
//!   `hsv_to_rgb(BLI_hash_int_01(hash), 0.5, 0.8)`, where `hash` is
//!   `BLI_ghashutil_strhash_p_murmur(ob->id.name)`: MurmurHash2 (`BLI_hash_mm2`, seed 0) of the
//!   `OB`-prefixed name **including its terminating NUL** (`strlen + 1` bytes), mixed with the
//!   library path for linked objects (verified against Blender renders). `RObject::random_hash` carries that `hash`; the
//!   renderer applies `BLI_hash_int_01` and the HSV conversion. Non-material colour types use
//!   metallic 0 and roughness 0.4; so does a slot without a material
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

#[cfg(test)]
mod tests {
    use super::*;

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
