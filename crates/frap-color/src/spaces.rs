// SPDX-License-Identifier: GPL-3.0-or-later
//! Colour spaces and transfer functions (sRGB, Linear Rec.709, Non-Color, …). WP 1.6.
//!
//! The transfer functions are the ones Blender 5.2.4's OCIO config uses
//! (`datafiles/colormanagement/config.ocio`), not textbook approximations:
//!
//! - `sRGB` is `ExponentWithLinearTransform {gamma: 2.4, offset: 0.055}`. OCIO derives the linear
//!   segment so that the curve is C1-continuous, which gives a break at encoded `0.0392857`
//!   (`offset / (gamma - 1)`) and a slope of `12.9232`, slightly different from IEC 61966-2-1's
//!   `0.04045` / `12.92`. Negative values follow OCIO's default `NEGATIVE_LINEAR` style (the
//!   linear segment is extended).
//! - `Rec.1886` is a pure 2.4 exponent (`ExponentTransform`, negatives clamped to 0).
//!
//! Every transcendental goes through `libm` (rule 2.2.12), so outputs are bit-identical across
//! platforms.

/// OCIO `ExponentWithLinearTransform` gamma of the sRGB display.
const SRGB_GAMMA: f64 = 2.4;
/// OCIO `ExponentWithLinearTransform` offset of the sRGB display.
const SRGB_OFFSET: f64 = 0.055;
/// Break point in encoded space: `offset / (gamma - 1)`.
const SRGB_EBREAK: f32 = (SRGB_OFFSET / (SRGB_GAMMA - 1.0)) as f32;
/// Break point in linear space: `((ebreak + offset) / (1 + offset))^gamma` = 0.0030399…
/// (computed in f64 by `srgb_constants_match_ocio` below; stored here as the f32 result).
const SRGB_LBREAK: f32 = 0.003_039_934_6;
/// Slope of the linear segment: `ebreak / lbreak` = 12.9232…
const SRGB_SLOPE: f32 = 12.923_21;

/// Rec.709 luma coefficients (`config.ocio`: `luma: [0.2126, 0.7152, 0.0722]`), used by
/// Blender's `IMB_colormanagement_get_luminance` for scene-linear Rec.709.
pub const LUMA_REC709: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// Linear Rec.709 → CIE XYZ (D65), the inverse of `config.ocio`'s "Linear Rec.709" matrix.
pub const REC709_TO_XYZ: [[f32; 3]; 3] = [
    [0.412_390_8, 0.357_584_33, 0.180_480_8],
    [0.212_639, 0.715_168_7, 0.072_192_32],
    [0.019_330_818, 0.119_194_78, 0.950_532_14],
];

/// Scene-linear → sRGB display encoding (OCIO `ExponentWithLinearTransform`, inverse
/// direction, `NEGATIVE_LINEAR`).
#[inline]
pub fn linear_to_srgb(x: f32) -> f32 {
    if x < SRGB_LBREAK {
        x * SRGB_SLOPE
    } else {
        (1.0 + SRGB_OFFSET as f32) * libm::powf(x, (1.0 / SRGB_GAMMA) as f32) - SRGB_OFFSET as f32
    }
}

/// sRGB display encoding → linear (OCIO `ExponentWithLinearTransform`, forward direction).
#[inline]
pub fn srgb_to_linear(e: f32) -> f32 {
    if e < SRGB_EBREAK {
        e / SRGB_SLOPE
    } else {
        libm::powf(
            (e + SRGB_OFFSET as f32) / (1.0 + SRGB_OFFSET as f32),
            SRGB_GAMMA as f32,
        )
    }
}

/// Rec.1886 (pure 2.4 exponent) encoded → linear; negatives clamp to 0 (OCIO
/// `ExponentTransform` default `NEGATIVE_CLAMP`).
#[inline]
pub fn rec1886_to_linear(e: f32) -> f32 {
    if e <= 0.0 { 0.0 } else { libm::powf(e, 2.4) }
}

/// Luminance of a scene-linear Rec.709 colour.
#[inline]
pub fn luminance(rgb: [f32; 3]) -> f32 {
    LUMA_REC709[0] * rgb[0] + LUMA_REC709[1] * rgb[1] + LUMA_REC709[2] * rgb[2]
}

/// 3×3 matrix times vector.
#[inline]
pub fn mat3_mul(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// The colour spaces of `ColorManagedInputColorspaceSettings.name` that frap understands
/// (Blender's names; aliases from `config.ocio` are accepted by [`ColorSpace::from_name`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColorSpace {
    /// "Linear Rec.709" (the scene-linear role, `default_float`).
    LinearRec709,
    /// "sRGB" (`default_byte`).
    Srgb,
    /// "Non-Color" (the data role: never converted).
    NonColor,
}

impl ColorSpace {
    /// Blender's name.
    pub fn name(self) -> &'static str {
        match self {
            ColorSpace::LinearRec709 => "Linear Rec.709",
            ColorSpace::Srgb => "sRGB",
            ColorSpace::NonColor => "Non-Color",
        }
    }

    /// Looks up a colour space by Blender name or `config.ocio` alias.
    pub fn from_name(name: &str) -> Option<ColorSpace> {
        match name {
            "Linear Rec.709"
            | "lin_rec709_scene"
            | "lin_rec709_display"
            | "Linear"
            | "Linear BT.709"
            | "Linear BT.709 I-D65"
            | "Linear Tristimulus"
            | "linrec709"
            | "Utility - Linear - sRGB"
            | "Utility - Linear - Rec.709"
            | "lin_srgb"
            | "Linear Rec.709 (sRGB)"
            | "lin_rec709_srgb"
            | "lin_rec709"
            | "CGI: Linear - Rec.709" => Some(ColorSpace::LinearRec709),
            "sRGB"
            | "srgb_rec709_display"
            | "srgb_rec709_scene"
            | "sRGB I-D65"
            | "srgb_display"
            | "sRGB - Display"
            | "Utility - sRGB - Texture"
            | "sRGB - Texture"
            | "srgb_tx"
            | "srgb_texture"
            | "Input - Generic - sRGB - Texture" => Some(ColorSpace::Srgb),
            "Non-Color" | "Raw" | "Generic Data" | "Non-Colour Data" | "Utility - Raw" | "raw" => {
                Some(ColorSpace::NonColor)
            }
            _ => None,
        }
    }

    /// `ColorSpace.is_data` (never colour-converted).
    pub fn is_data(self) -> bool {
        matches!(self, ColorSpace::NonColor)
    }

    /// Converts one channel value from this space to scene-linear Rec.709.
    #[inline]
    pub fn to_scene_linear(self, v: f32) -> f32 {
        match self {
            ColorSpace::LinearRec709 | ColorSpace::NonColor => v,
            ColorSpace::Srgb => srgb_to_linear(v),
        }
    }

    /// Converts one channel value from scene-linear Rec.709 to this space.
    #[inline]
    pub fn from_scene_linear(self, v: f32) -> f32 {
        match self {
            ColorSpace::LinearRec709 | ColorSpace::NonColor => v,
            ColorSpace::Srgb => linear_to_srgb(v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_constants_match_ocio() {
        let ebreak = SRGB_OFFSET / (SRGB_GAMMA - 1.0);
        let lbreak = ((ebreak + SRGB_OFFSET) / (1.0 + SRGB_OFFSET)).powf(SRGB_GAMMA);
        assert_eq!(SRGB_LBREAK, lbreak as f32);
        assert!(((ebreak / lbreak) as f32 - SRGB_SLOPE).abs() < 1e-4);
    }

    #[test]
    fn srgb_round_trip_and_continuity() {
        for i in 0..=1000 {
            let x = i as f32 / 1000.0;
            let back = srgb_to_linear(linear_to_srgb(x));
            assert!((back - x).abs() < 2e-6, "{x} -> {back}");
        }
        // Continuous at the break.
        let below = linear_to_srgb(SRGB_LBREAK * (1.0 - 1e-6));
        let above = linear_to_srgb(SRGB_LBREAK);
        assert!((below - above).abs() < 1e-5);
        assert_eq!(linear_to_srgb(0.0), 0.0);
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-6);
        // 18 % grey encodes to ~0.4613 (IEC sRGB gives 0.4613 too).
        assert!((linear_to_srgb(0.18) - 0.461_356).abs() < 1e-4);
        // NEGATIVE_LINEAR.
        assert!((linear_to_srgb(-0.001) + 0.001 * SRGB_SLOPE).abs() < 1e-9);
    }

    #[test]
    fn colour_space_names_and_aliases() {
        assert_eq!(ColorSpace::from_name("sRGB"), Some(ColorSpace::Srgb));
        assert_eq!(
            ColorSpace::from_name("Linear"),
            Some(ColorSpace::LinearRec709)
        );
        assert_eq!(
            ColorSpace::from_name("Non-Color"),
            Some(ColorSpace::NonColor)
        );
        assert_eq!(ColorSpace::from_name("ACEScg"), None);
        for cs in [
            ColorSpace::Srgb,
            ColorSpace::LinearRec709,
            ColorSpace::NonColor,
        ] {
            assert_eq!(ColorSpace::from_name(cs.name()), Some(cs));
        }
        assert!(ColorSpace::NonColor.is_data());
        assert_eq!(ColorSpace::Srgb.to_scene_linear(1.0), 1.0);
        assert!((ColorSpace::Srgb.from_scene_linear(0.5) - 0.735_357).abs() < 1e-4);
    }

    #[test]
    fn rec709_white_is_d65() {
        let xyz = mat3_mul(&REC709_TO_XYZ, [1.0, 1.0, 1.0]);
        assert!((xyz[0] - 0.950_456).abs() < 1e-4, "{xyz:?}");
        assert!((xyz[1] - 1.0).abs() < 1e-4);
        assert!((xyz[2] - 1.089_058).abs() < 1e-3);
        assert!((luminance([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
    }
}
