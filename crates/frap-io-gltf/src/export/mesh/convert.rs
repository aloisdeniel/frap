// SPDX-License-Identifier: GPL-3.0-or-later
//! Value conversions shared by extractors and attribute writers, each the add-on's exact rule:
//! the Y-up swizzle, the split-key float encoding, the byte order of `fast_structured_np_unique`
//! and the colour encodings (BYTE_COLOR → normalized UNSIGNED_SHORT).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`zup2yup`), blender/com/gltf2_blender_utils.py (`fast_structured_np_unique`) and
//! blender/exp/primitive_attributes.py (`__gather_attribute`, the UNSIGNED_SHORT colour branch);
//! the sRGB curve from blender/source/blender/blenlib/intern/math_color.cc
//! (`srgb_to_linearrgb`, `linearrgb_to_srgb`).

use std::cmp::Ordering;

pub use frap_mesh::split::{normal_key, round4};

/// Blender (Z-up) → glTF (Y-up): `(x, y, z) → (x, z, -y)` (the add-on's `zup2yup`; `-y` of a
/// zero is `-0.0`, as numpy's `*= -1` gives).
pub fn yup(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], -v[1]]
}

/// [`yup`] when `export_yup`, else the value unchanged.
pub fn axis(v: [f32; 3], export_yup: bool) -> [f32; 3] {
    if export_yup { yup(v) } else { v }
}

/// `-0.0` → `0.0` (`fast_structured_np_unique` folds negative zeros before comparing bytes).
pub fn fold_zero(x: f32) -> f32 {
    if x == 0.0 { 0.0 } else { x }
}

/// A float as one split-key word: its bits after [`fold_zero`].
pub fn key_word(x: f32) -> u32 {
    fold_zero(x).to_bits()
}

/// The order in which `fast_structured_np_unique` sorts records of `words.len()` little-endian
/// u32 words (`vertex_index` first): a 4-byte record sorts as a `uint32`, an 8-byte record as a
/// little-endian `uint64` (its second word is the most significant), and any longer record as a
/// UCS4 string, i.e. word by word.
pub fn record_cmp(a: &[u32], b: &[u32]) -> Ordering {
    if a.len() == 2 && b.len() == 2 {
        (a[1], a[0]).cmp(&(b[1], b[0]))
    } else {
        a.cmp(b)
    }
}

/// `srgb_to_linearrgb` (one channel).
pub fn srgb_to_linear(c: f32) -> f32 {
    if c < 0.04045 {
        if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) }
    } else {
        libm::powf((c + 0.055) * (1.0 / 1.055), 2.4)
    }
}

/// `linearrgb_to_srgb` (one channel).
pub fn linear_to_srgb(c: f32) -> f32 {
    if c < 0.003_130_8 {
        if c < 0.0 { 0.0 } else { c * 12.92 }
    } else {
        1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
    }
}

/// `unit_float_to_uchar_clamp`: `[0, 1]` → byte, rounding half up.
pub fn unit_to_u8(c: f32) -> u8 {
    if c <= 0.0 {
        0
    } else if c > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * c + 0.5) as u8
    }
}

/// A BYTE_COLOR attribute value as RNA's `.color` reads it (what the add-on exports): sRGB
/// bytes decoded to linear RGB (`BLI_color_from_srgb_table`), alpha divided by 255.
pub fn byte_color_to_linear(c: [u8; 4]) -> [f32; 4] {
    let f = |b: u8| f32::from(b) * (1.0 / 255.0);
    [
        srgb_to_linear(f(c[0])),
        srgb_to_linear(f(c[1])),
        srgb_to_linear(f(c[2])),
        f(c[3]),
    ]
}

/// A colour as normalized UNSIGNED_SHORT: the add-on writes BYTE_COLOR attributes this way
/// (not as bytes): clipped to `[0, 1]`, `× 65535 + 0.5` in float32, truncated.
pub fn color_to_ushort(c: [f32; 4]) -> [u16; 4] {
    c.map(|x| {
        let x = if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        (x * 65535.0 + 0.5) as u16
    })
}

/// BYTE_COLOR value → the exported UNSIGNED_SHORT VEC4 element.
pub fn byte_color_to_ushort(c: [u8; 4]) -> [u16; 4] {
    color_to_ushort(byte_color_to_linear(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yup_swizzle() {
        assert_eq!(yup([1.0, 2.0, 3.0]), [1.0, 3.0, -2.0]);
        assert_eq!(axis([1.0, 2.0, 3.0], false), [1.0, 2.0, 3.0]);
        // numpy's `*= -1` turns +0 into -0; the key folds it back.
        assert!(yup([0.0, 0.0, 0.0])[2].is_sign_negative());
        assert_eq!(key_word(-0.0), 0);
    }

    #[test]
    fn normals_rounded_to_4_decimals_then_normalised() {
        assert_eq!(round4(0.123_456), 0.1235);
        assert_eq!(round4(-0.000_04), -0.0);
        let k = normal_key([0.577_350_26, 0.577_350_26, 0.577_350_26]);
        let r = 0.5774f32;
        let len = (r * r + r * r + r * r).sqrt();
        assert_eq!(k, [r / len; 3]);
        // A zero normal becomes +Z.
        assert_eq!(normal_key([0.0, 0.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(normal_key([0.000_01, 0.0, 0.0]), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn record_order_is_numpys() {
        // 4-byte records: uint32.
        assert_eq!(record_cmp(&[2], &[10]), Ordering::Less);
        // 8-byte records: little-endian uint64, the second word is most significant.
        assert_eq!(record_cmp(&[5, 1], &[1, 2]), Ordering::Less);
        assert_eq!(record_cmp(&[1, 2], &[5, 1]), Ordering::Greater);
        // Longer records: UCS4 strings, word by word.
        assert_eq!(record_cmp(&[1, 9, 9], &[2, 0, 0]), Ordering::Less);
        assert_eq!(
            record_cmp(&[1, 0x8000_0000, 0], &[1, 0x3F80_0000, 0]),
            Ordering::Greater
        );
    }

    #[test]
    fn byte_color_exports_as_ushort() {
        // White and black are exact; alpha is linear.
        assert_eq!(byte_color_to_ushort([255, 255, 255, 255]), [65535; 4]);
        assert_eq!(byte_color_to_ushort([0, 0, 0, 0]), [0; 4]);
        // sRGB 128 → linear 0.21586 → 14146.
        let lin = srgb_to_linear(128.0 / 255.0);
        assert!((lin - 0.215_861_05).abs() < 1e-6);
        assert_eq!(
            byte_color_to_ushort([128, 128, 128, 128])[..3],
            [(lin * 65535.0 + 0.5) as u16; 3]
        );
        assert_eq!(byte_color_to_ushort([0, 0, 0, 128])[3], 32896);
        // Out-of-range floats are clipped.
        assert_eq!(
            color_to_ushort([2.0, -1.0, 0.5, f32::NAN]),
            [65535, 0, 32768, 0]
        );
    }

    #[test]
    fn srgb_round_trip() {
        for b in [0u8, 1, 10, 64, 128, 200, 255] {
            let lin = srgb_to_linear(f32::from(b) / 255.0);
            assert_eq!(unit_to_u8(linear_to_srgb(lin)), b);
        }
    }
}
