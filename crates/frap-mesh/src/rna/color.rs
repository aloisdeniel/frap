// SPDX-License-Identifier: GPL-3.0-or-later
//! Colour conversions of the colour-attribute RNA (`ByteColorAttributeValue.color/color_srgb`,
//! `FloatColorAttributeValue.color_srgb`, `MeshLoopColor.color`).
//!
//! Byte colours store sRGB-encoded bytes; `color` decodes them to scene linear with Blender's
//! `BLI_color_from_srgb_table` (the C `srgb_to_linearrgb` of `i / 255`), alpha linear, and
//! encodes with `linearrgb_to_srgb_uchar4` (C `linearrgb_to_srgb`, then
//! `unit_float_to_uchar_clamp`). These match Blender 5.2.4 bit for bit (tested).
//!
//! `FloatColorAttributeValue.color_srgb` goes through OCIO in Blender
//! (`IMB_colormanagement_scene_linear_to_srgb_v3`), whose CPU processor approximates the power
//! function: frap uses the exact C formula, which differs by up to ~3e-5 (1.0 reads
//! 0.99997 in Blender, 1.0 in frap).

/// `powf` correctly rounded: evaluated in `f64` (`libm::pow`, rule 2.2.12) and rounded once,
/// which matches the platform `powf` Blender calls (1 ULP closer than `libm::powf` on the
/// byte table, verified on `srgb_to_linearrgb(64 / 255)`).
fn powf(x: f32, y: f32) -> f32 {
    libm::pow(f64::from(x), f64::from(y)) as f32
}

// Ported from blender/source/blender/blenlib/intern/math_color_inline.cc (srgb_to_linearrgb).
/// sRGB-encoded → scene linear (IEC 61966-2-1 as Blender's C code writes it).
pub(crate) fn srgb_to_linear(c: f32) -> f32 {
    if c < 0.04045 {
        if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) }
    } else {
        powf((c + 0.055) * (1.0 / 1.055), 2.4)
    }
}

// Ported from blender/source/blender/blenlib/intern/math_color_inline.cc (linearrgb_to_srgb).
/// Scene linear → sRGB-encoded.
pub(crate) fn linear_to_srgb(c: f32) -> f32 {
    if c < 0.003_130_8 {
        if c < 0.0 { 0.0 } else { c * 12.92 }
    } else {
        1.055 * powf(c, 1.0 / 2.4) - 0.055
    }
}

// Ported from blender/source/blender/blenlib/BLI_math_base.h (unit_float_to_uchar_clamp).
/// `[0, 1]` float → byte, rounded, clamped.
pub(crate) fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// Byte → `[0, 1]` float (`b / 255`; verified: 7 reads 0.02745098, not `7 * (1 / 255)`).
pub(crate) fn uchar_to_float(b: u8) -> f32 {
    f32::from(b) / 255.0
}

/// `ByteColorAttributeValue.color` read: sRGB bytes → linear RGB, linear alpha
/// (`srgb_to_linearrgb_uchar4`: `BLI_color_from_srgb_table[b]`, built from
/// `srgb_to_linearrgb(b * (1 / 255))`, and alpha `b * (1 / 255)`; verified bit for bit).
pub(crate) fn byte_to_linear(c: [u8; 4]) -> [f32; 4] {
    let unit = |b: u8| f32::from(b) * (1.0 / 255.0);
    [
        srgb_to_linear(unit(c[0])),
        srgb_to_linear(unit(c[1])),
        srgb_to_linear(unit(c[2])),
        unit(c[3]),
    ]
}

/// `ByteColorAttributeValue.color` write: linear RGB → sRGB bytes, linear alpha.
pub(crate) fn linear_to_byte(c: [f32; 4]) -> [u8; 4] {
    [
        unit_float_to_uchar_clamp(linear_to_srgb(c[0])),
        unit_float_to_uchar_clamp(linear_to_srgb(c[1])),
        unit_float_to_uchar_clamp(linear_to_srgb(c[2])),
        unit_float_to_uchar_clamp(c[3]),
    ]
}

/// Bytes → floats without colour conversion (`color_srgb`, `MeshLoopColor.color`).
pub(crate) fn byte_to_float(c: [u8; 4]) -> [f32; 4] {
    c.map(uchar_to_float)
}

/// Floats → bytes without colour conversion.
pub(crate) fn float_to_byte(c: [f32; 4]) -> [u8; 4] {
    c.map(unit_float_to_uchar_clamp)
}

/// `FloatColorAttributeValue.color_srgb` read.
pub(crate) fn linear4_to_srgb(c: [f32; 4]) -> [f32; 4] {
    [
        linear_to_srgb(c[0]),
        linear_to_srgb(c[1]),
        linear_to_srgb(c[2]),
        c[3],
    ]
}

/// `FloatColorAttributeValue.color_srgb` write.
pub(crate) fn srgb4_to_linear(c: [f32; 4]) -> [f32; 4] {
    [
        srgb_to_linear(c[0]),
        srgb_to_linear(c[1]),
        srgb_to_linear(c[2]),
        c[3],
    ]
}
