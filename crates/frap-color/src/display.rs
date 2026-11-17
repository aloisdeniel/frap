// SPDX-License-Identifier: GPL-3.0-or-later
//! `display_transform()`: exposure, view transform, look, gamma, display encode, dither
//! (`render.dither_intensity`). WP 1.6.
//!
//! The order is Blender 5.2.4's display processor
//! (`source/blender/imbuf/opencolorio/intern/libocio/libocio_display_processor.cc`,
//! `create_ocio_display_processor`, GPL-2.0-or-later):
//!
//! 1. exposure: scene-linear RGB × `powf(2, exposure)` (skipped when `exposure == 0`);
//! 2. look (when it applies to the view, `colormanage_use_look`) + view + display transform;
//! 3. gamma: `ExponentTransform` with exponent `min(1 / max(FLT_EPSILON, gamma), 100)`
//!    (negatives clamp to 0), skipped when the exponent is 1.
//!
//! Byte output then goes through `IMB_buffer_byte_from_float`, whose dither
//! (`float_to_byte_dither_v4` + `dither_random_value`) is ported in [`DisplayImage::to_rgba8`].
//! Alpha is straight and passes through unchanged (Blender un-premultiplies around the
//! processor, so straight input gives the same result).

use crate::lut::{self, ViewLut};
use crate::spaces::linear_to_srgb;

/// `view_settings.view_transform` items of Blender 5.2 (sRGB display order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewTransform {
    /// "Standard": the display's transfer function only (analytic in frap).
    Standard,
    /// "ACES 1.3".
    Aces13,
    /// "ACES 2.0".
    Aces20,
    /// "Khronos PBR Neutral".
    KhronosPbrNeutral,
    /// "AgX" (factory default).
    AgX,
    /// "Filmic".
    Filmic,
    /// "Filmic Log".
    FilmicLog,
    /// "False Color".
    FalseColor,
    /// "Raw": no conversion (analytic in frap).
    Raw,
}

impl ViewTransform {
    /// Every view transform, in Blender's enum order.
    pub const ALL: [ViewTransform; 9] = [
        ViewTransform::Standard,
        ViewTransform::Aces13,
        ViewTransform::Aces20,
        ViewTransform::KhronosPbrNeutral,
        ViewTransform::AgX,
        ViewTransform::Filmic,
        ViewTransform::FilmicLog,
        ViewTransform::FalseColor,
        ViewTransform::Raw,
    ];

    /// Blender's identifier (`view_settings.view_transform` value).
    pub fn name(self) -> &'static str {
        match self {
            ViewTransform::Standard => "Standard",
            ViewTransform::Aces13 => "ACES 1.3",
            ViewTransform::Aces20 => "ACES 2.0",
            ViewTransform::KhronosPbrNeutral => "Khronos PBR Neutral",
            ViewTransform::AgX => "AgX",
            ViewTransform::Filmic => "Filmic",
            ViewTransform::FilmicLog => "Filmic Log",
            ViewTransform::FalseColor => "False Color",
            ViewTransform::Raw => "Raw",
        }
    }

    /// Parses Blender's identifier.
    pub fn from_name(name: &str) -> Option<ViewTransform> {
        ViewTransform::ALL.into_iter().find(|v| v.name() == name)
    }
}

/// `display_settings.display_device` items of Blender 5.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DisplayDevice {
    /// "sRGB" (factory default).
    Srgb,
    /// "Display P3".
    DisplayP3,
    /// "Rec.1886".
    Rec1886,
    /// "Rec.2020".
    Rec2020,
    /// "Rec.2100-PQ".
    Rec2100Pq,
    /// "Rec.2100-HLG".
    Rec2100Hlg,
}

impl DisplayDevice {
    /// Every display, in Blender's order (`active_displays`).
    pub const ALL: [DisplayDevice; 6] = [
        DisplayDevice::Srgb,
        DisplayDevice::DisplayP3,
        DisplayDevice::Rec1886,
        DisplayDevice::Rec2020,
        DisplayDevice::Rec2100Pq,
        DisplayDevice::Rec2100Hlg,
    ];

    /// Blender's identifier.
    pub fn name(self) -> &'static str {
        match self {
            DisplayDevice::Srgb => "sRGB",
            DisplayDevice::DisplayP3 => "Display P3",
            DisplayDevice::Rec1886 => "Rec.1886",
            DisplayDevice::Rec2020 => "Rec.2020",
            DisplayDevice::Rec2100Pq => "Rec.2100-PQ",
            DisplayDevice::Rec2100Hlg => "Rec.2100-HLG",
        }
    }

    /// Parses Blender's identifier.
    pub fn from_name(name: &str) -> Option<DisplayDevice> {
        DisplayDevice::ALL.into_iter().find(|d| d.name() == name)
    }
}

/// The looks of Blender 5.2.4's `config.ocio` (besides "None"). A look named `"<View> - <Name>"`
/// applies only to the view `<View>`; the others apply to views that have no view-specific look.
pub const LOOKS: &[&str] = &[
    "Very High Contrast",
    "High Contrast",
    "Medium High Contrast",
    "Medium Contrast",
    "Medium Low Contrast",
    "Low Contrast",
    "Very Low Contrast",
    "AgX - Punchy",
    "AgX - Greyscale",
    "AgX - Very High Contrast",
    "AgX - High Contrast",
    "AgX - Medium High Contrast",
    "AgX - Base Contrast",
    "AgX - Medium Low Contrast",
    "AgX - Low Contrast",
    "AgX - Very Low Contrast",
    "False Color - Punchy",
    "False Color - Greyscale",
    "False Color - Very High Contrast",
    "False Color - High Contrast",
    "False Color - Medium High Contrast",
    "False Color - Base Contrast",
    "False Color - Medium Low Contrast",
    "False Color - Low Contrast",
    "False Color - Very Low Contrast",
    "ACES 1.3 - Reference Gamut Compression",
    "ACES 2.0 - Reference Gamut Compression",
];

fn look_view(look: &str) -> &str {
    look.split_once(" - ").map(|(v, _)| v).unwrap_or("")
}

/// Whether Blender applies `look` with `view` (port of `colormanage_use_look` and
/// `view_filter_for_look`, `imbuf/intern/colormanagement.cc`). `"None"` never applies.
pub fn look_applies(view: ViewTransform, look: &str) -> Result<bool, ColorError> {
    if look == "None" {
        return Ok(false);
    }
    if !LOOKS.contains(&look) {
        return Err(ColorError::UnknownLook(look.to_owned()));
    }
    let name = view.name();
    let filter = if LOOKS.iter().any(|l| look_view(l) == name) {
        name
    } else {
        let short = name.split_once(" - ").map(|(s, _)| s).unwrap_or("");
        if !short.is_empty() && LOOKS.iter().any(|l| look_view(l) == short) {
            short
        } else {
            ""
        }
    };
    Ok(look_view(look) == filter)
}

/// `scene.view_settings` (the parts frap applies).
#[derive(Clone, Debug, PartialEq)]
pub struct ViewSettings {
    /// `view_transform`.
    pub view_transform: ViewTransform,
    /// `look` (`"None"` or one of [`LOOKS`]).
    pub look: String,
    /// `exposure` in stops.
    pub exposure: f32,
    /// `gamma`.
    pub gamma: f32,
}

impl Default for ViewSettings {
    /// Factory values: AgX, look None, exposure 0, gamma 1.
    fn default() -> Self {
        ViewSettings {
            view_transform: ViewTransform::AgX,
            look: "None".to_owned(),
            exposure: 0.0,
            gamma: 1.0,
        }
    }
}

/// `scene.display_settings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplaySettings {
    /// `display_device`.
    pub display_device: DisplayDevice,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        DisplaySettings {
            display_device: DisplayDevice::Srgb,
        }
    }
}

/// Colour-management failure.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorError {
    /// `look` is not a look of Blender 5.2's config.
    UnknownLook(String),
    /// The combination exists in Blender but frap has no implementation (no baked LUT) yet.
    NotAvailable {
        /// View transform name.
        view_transform: &'static str,
        /// Look name.
        look: String,
        /// Display name.
        display: &'static str,
    },
}

impl std::fmt::Display for ColorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorError::UnknownLook(l) => write!(f, "unknown look '{l}'"),
            ColorError::NotAvailable {
                view_transform,
                look,
                display,
            } => write!(
                f,
                "view transform '{view_transform}' with look '{look}' on display '{display}' \
                 is not available in frap yet"
            ),
        }
    }
}

impl std::error::Error for ColorError {}

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// Standard on sRGB: OCIO's piecewise sRGB encode.
    StandardSrgb,
    /// Raw: the data colour space, no conversion.
    Raw,
    /// A baked LUT.
    Lut(&'static ViewLut),
}

/// A ready-to-apply display transform (Blender's display `CPUProcessor`).
#[derive(Clone, Debug)]
pub struct DisplayProcessor {
    scale: f32,
    exponent: f32,
    kind: Kind,
}

impl DisplayProcessor {
    /// Builds the processor for `view` on `display`.
    ///
    /// Errors with [`ColorError::NotAvailable`] for combinations frap cannot reproduce yet
    /// (M1: Standard and Raw on every display are analytic only on sRGB; AgX without a look is
    /// baked for sRGB). Callers may fall back to Standard with a warning.
    pub fn new(view: &ViewSettings, display: &DisplaySettings) -> Result<Self, ColorError> {
        let use_look = look_applies(view.view_transform, &view.look)?;
        let not_available = || ColorError::NotAvailable {
            view_transform: view.view_transform.name(),
            look: view.look.clone(),
            display: display.display_device.name(),
        };
        let kind = match (view.view_transform, display.display_device, use_look) {
            // Raw is the data colour space: looks are skipped (`create_ocio_display_transform`).
            (ViewTransform::Raw, _, _) => Kind::Raw,
            (ViewTransform::Standard, DisplayDevice::Srgb, false) => Kind::StandardSrgb,
            (vt, dd, look) => {
                let look_name = if look { view.look.as_str() } else { "None" };
                Kind::Lut(lut::baked(vt.name(), look_name, dd.name()).ok_or_else(not_available)?)
            }
        };
        let scale = if view.exposure == 0.0 {
            1.0
        } else {
            libm::powf(2.0, view.exposure)
        };
        let exponent = (1.0 / view.gamma.max(f32::EPSILON)).min(100.0);
        Ok(DisplayProcessor {
            scale,
            exponent,
            kind,
        })
    }

    /// Scene-linear Rec.709 → display-encoded RGB (not clamped; files clamp on write).
    #[inline]
    pub fn apply_rgb(&self, rgb: [f32; 3]) -> [f32; 3] {
        let x = if self.scale == 1.0 {
            rgb
        } else {
            rgb.map(|v| v * self.scale)
        };
        let d = match self.kind {
            Kind::StandardSrgb => x.map(linear_to_srgb),
            Kind::Raw => x,
            Kind::Lut(l) => l.apply(x),
        };
        if self.exponent == 1.0 {
            d
        } else {
            d.map(|v| {
                if v > 0.0 {
                    libm::powf(v, self.exponent)
                } else {
                    0.0
                }
            })
        }
    }

    /// Applies to straight-alpha RGBA (alpha unchanged).
    #[inline]
    pub fn apply_rgba(&self, p: [f32; 4]) -> [f32; 4] {
        let [r, g, b] = self.apply_rgb([p[0], p[1], p[2]]);
        [r, g, b, p[3]]
    }
}

/// A display-referred image: display-encoded RGB, straight alpha, top row first, unclamped.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height` pixels, top row first.
    pub rgba: Vec<[f32; 4]>,
}

impl DisplayImage {
    /// Quantises to 8-bit RGBA like `IMB_buffer_byte_from_float` with
    /// `render.dither_intensity` (0 disables dithering). Blender's dither noise is keyed by the
    /// pixel's (column, row-from-bottom), so the result equals Blender's for the same buffer.
    pub fn to_rgba8(&self, dither_intensity: f32) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.rgba.len() * 4);
        let w = self.width as usize;
        for (i, p) in self.rgba.iter().enumerate() {
            if dither_intensity != 0.0 {
                let x = (i % w) as i32;
                let y = self.height as i32 - 1 - (i / w) as i32;
                let d = dither_random_value(x, y) * 0.0033 * dither_intensity;
                out.extend_from_slice(&[
                    unit_float_to_uchar_clamp(d + p[0]),
                    unit_float_to_uchar_clamp(d + p[1]),
                    unit_float_to_uchar_clamp(d + p[2]),
                    unit_float_to_uchar_clamp(p[3]),
                ]);
            } else {
                out.extend(p.map(unit_float_to_uchar_clamp));
            }
        }
        out
    }

    /// Quantises to 16-bit RGBA (`unit_float_to_ushort_clamp`; no dither, as Blender's 16-bit
    /// writers take the float buffer directly).
    pub fn to_rgba16(&self) -> Vec<u16> {
        self.rgba
            .iter()
            .flat_map(|p| p.map(unit_float_to_ushort_clamp))
            .collect()
    }
}

/// Colour-manages a scene-linear, straight-alpha RGBA film (top row first) for display.
pub fn display_transform(
    rgba: &[[f32; 4]],
    width: u32,
    height: u32,
    view: &ViewSettings,
    display: &DisplaySettings,
) -> Result<DisplayImage, ColorError> {
    assert_eq!(
        rgba.len(),
        width as usize * height as usize,
        "display_transform: pixel count does not match width × height"
    );
    let p = DisplayProcessor::new(view, display)?;
    Ok(DisplayImage {
        width,
        height,
        rgba: rgba.iter().map(|px| p.apply_rgba(*px)).collect(),
    })
}

/// Blender's `unit_float_to_uchar_clamp` (`BLI_math_base_inline`); NaN → 0.
#[inline]
pub fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 || v.is_nan() {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// Blender's `unit_float_to_ushort_clamp`; NaN → 0.
#[inline]
pub fn unit_float_to_ushort_clamp(v: f32) -> u16 {
    if v >= 1.0 - 0.5 / 65535.0 {
        65535
    } else if v <= 0.0 || v.is_nan() {
        0
    } else {
        (v * 65535.0 + 0.5) as u16
    }
}

/// Blender's `hash_iqint3_f` (`BLI_math_color_inline`): 2D integer hash → `[0, 1]`.
#[inline]
fn hash_iqint3_f(x: u32, y: u32) -> f32 {
    let qx = 1_103_515_245u32.wrapping_mul((x >> 1) ^ y);
    let qy = 1_103_515_245u32.wrapping_mul((y >> 1) ^ x);
    let n = 1_103_515_245u32.wrapping_mul(qx ^ (qy >> 3));
    n as f32 * (1.0 / u32::MAX as f32)
}

/// Blender's `dither_random_value(x, y)`: triangle-distributed noise in `(-1, 1)`.
#[inline]
pub fn dither_random_value(x: i32, y: i32) -> f32 {
    let v = hash_iqint3_f(x as u32, y as u32) * 2.0 - 1.0;
    let sign = if v < 0.0 { -1.0 } else { 1.0 };
    sign * (1.0 - (1.0 - v.abs()).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for v in ViewTransform::ALL {
            assert_eq!(ViewTransform::from_name(v.name()), Some(v));
        }
        for d in DisplayDevice::ALL {
            assert_eq!(DisplayDevice::from_name(d.name()), Some(d));
        }
        assert_eq!(ViewTransform::from_name("Filmic Base Contrast"), None);
        assert_eq!(ViewSettings::default().view_transform, ViewTransform::AgX);
    }

    #[test]
    fn look_rules_follow_blender() {
        use ViewTransform::*;
        assert!(!look_applies(AgX, "None").unwrap());
        assert!(look_applies(AgX, "AgX - Punchy").unwrap());
        // View-specific looks only for their view; generic looks never on a view that has some.
        assert!(!look_applies(Standard, "AgX - Punchy").unwrap());
        assert!(!look_applies(AgX, "High Contrast").unwrap());
        // Generic (Filmic) looks apply to views without view-specific looks.
        assert!(look_applies(Filmic, "High Contrast").unwrap());
        assert!(look_applies(Standard, "High Contrast").unwrap());
        assert!(look_applies(FalseColor, "False Color - Punchy").unwrap());
        assert!(look_applies(Aces13, "ACES 1.3 - Reference Gamut Compression").unwrap());
        assert!(matches!(
            look_applies(AgX, "Punchy"),
            Err(ColorError::UnknownLook(_))
        ));
    }

    #[test]
    fn processor_availability() {
        let d = DisplaySettings::default();
        let mut v = ViewSettings::default();
        assert!(DisplayProcessor::new(&v, &d).is_ok());
        v.look = "AgX - Punchy".into();
        let e = DisplayProcessor::new(&v, &d).unwrap_err();
        assert_eq!(
            e.to_string(),
            "view transform 'AgX' with look 'AgX - Punchy' on display 'sRGB' is not available in frap yet"
        );
        // A look that does not apply to the view is ignored, as in Blender.
        v.look = "High Contrast".into();
        assert!(DisplayProcessor::new(&v, &d).is_ok());
        v.view_transform = ViewTransform::Filmic;
        v.look = "None".into();
        assert!(DisplayProcessor::new(&v, &d).is_err());
        v.view_transform = ViewTransform::Raw;
        v.look = "AgX - Punchy".into();
        assert!(DisplayProcessor::new(&v, &d).is_ok());
        let p3 = DisplaySettings {
            display_device: DisplayDevice::DisplayP3,
        };
        v.view_transform = ViewTransform::Standard;
        v.look = "None".into();
        assert!(DisplayProcessor::new(&v, &p3).is_err());
    }

    #[test]
    fn standard_raw_exposure_gamma() {
        let d = DisplaySettings::default();
        let mut v = ViewSettings {
            view_transform: ViewTransform::Standard,
            ..ViewSettings::default()
        };
        let p = DisplayProcessor::new(&v, &d).unwrap();
        assert!((p.apply_rgb([0.18; 3])[0] - linear_to_srgb(0.18)).abs() < 1e-7);
        v.exposure = 1.0;
        v.gamma = 2.0;
        let p = DisplayProcessor::new(&v, &d).unwrap();
        let want = libm::powf(linear_to_srgb(0.36), 0.5);
        assert!((p.apply_rgb([0.18; 3])[0] - want).abs() < 1e-6);
        // Negative display values clamp before the exponent.
        assert_eq!(p.apply_rgb([-1.0, 0.0, 0.0])[0], 0.0);
        v.view_transform = ViewTransform::Raw;
        v.exposure = 0.0;
        v.gamma = 1.0;
        let p = DisplayProcessor::new(&v, &d).unwrap();
        assert_eq!(p.apply_rgba([0.25, 2.0, -1.0, 0.5]), [0.25, 2.0, -1.0, 0.5]);
        // gamma 0 is guarded by FLT_EPSILON and capped at exponent 100.
        v.gamma = 0.0;
        let p = DisplayProcessor::new(&v, &d).unwrap();
        assert_eq!(p.exponent, 100.0);
    }

    #[test]
    fn quantisation_matches_blender_rounding() {
        assert_eq!(unit_float_to_uchar_clamp(-0.1), 0);
        assert_eq!(unit_float_to_uchar_clamp(f32::NAN), 0);
        assert_eq!(unit_float_to_uchar_clamp(0.5), 128);
        assert_eq!(unit_float_to_uchar_clamp(1.0 - 0.4 / 255.0), 255);
        assert_eq!(unit_float_to_uchar_clamp(1.0 / 255.0 * 0.49), 0);
        assert_eq!(unit_float_to_ushort_clamp(0.5), 32768);
        assert_eq!(unit_float_to_ushort_clamp(2.0), 65535);
        let img = DisplayImage {
            width: 2,
            height: 1,
            rgba: vec![[0.0, 0.5, 1.0, 1.0], [0.25, 0.75, 2.0, 0.0]],
        };
        assert_eq!(img.to_rgba8(0.0), vec![0, 128, 255, 255, 64, 191, 255, 0]);
        assert_eq!(img.to_rgba16()[1], 32768);
    }

    #[test]
    fn dither_is_triangular_bounded_and_deterministic() {
        let mut sum = 0.0f64;
        let mut n = 0;
        for y in 0..64 {
            for x in 0..64 {
                let v = dither_random_value(x, y);
                assert!((-1.0..=1.0).contains(&v));
                assert_eq!(v, dither_random_value(x, y));
                sum += f64::from(v);
                n += 1;
            }
        }
        assert!(
            (sum / f64::from(n)).abs() < 0.05,
            "mean {}",
            sum / f64::from(n)
        );
        // Dither of intensity 1 moves values by at most 0.0033, i.e. at most one 8-bit code.
        let img = DisplayImage {
            width: 64,
            height: 64,
            rgba: vec![[0.5, 0.2, 0.9, 1.0]; 64 * 64],
        };
        let plain = img.to_rgba8(0.0);
        let dith = img.to_rgba8(1.0);
        assert!(plain.iter().zip(&dith).all(|(a, b)| a.abs_diff(*b) <= 1));
        assert_ne!(plain, dith);
    }
}
