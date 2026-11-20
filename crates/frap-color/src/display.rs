// SPDX-License-Identifier: GPL-3.0-or-later
//! `display_transform()`: exposure, view transform, look, gamma, display encode, dither
//! (`render.dither_intensity`). WP 1.6; every view transform × look × SDR display by WP 5.3.
//!
//! The order is Blender 5.2.4's display processor
//! (`source/blender/imbuf/opencolorio/intern/libocio/libocio_display_processor.cc`,
//! `create_ocio_display_processor`, GPL-2.0-or-later):
//!
//! 1. exposure: scene-linear RGB × `powf(2, exposure)` (skipped when `exposure == 0`);
//! 2. look (when it applies to the view, `colormanage_use_look`) + view + display transform:
//!    the OCIO op pipeline baked from Blender for that display × view × look (`display/ocio.rs`,
//!    `assets/luts/*.ops`);
//! 3. gamma: `ExponentTransform` with exponent `min(1 / max(FLT_EPSILON, gamma), 100)`
//!    (negatives clamp to 0; OCIO's fast `ssePower`), skipped when the exponent is 1.
//!
//! Byte output then goes through `IMB_buffer_byte_from_float`, whose dither
//! (`float_to_byte_dither_v4` + `dither_random_value`) is ported in [`DisplayImage::to_rgba8`].
//! Alpha is straight and passes through unchanged (Blender un-premultiplies around the
//! processor, so straight input gives the same result).

pub(crate) mod ocio;

use ocio::Pipeline;

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

pub use crate::looks::{LOOKS, look_applies};

/// The Standard and Raw op pipelines (every SDR display, every look Blender applies to them).
pub(crate) const STANDARD_PIPELINES: ocio::OpsFile =
    include_str!("../../../assets/luts/standard.ops");

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
    /// The combination has no frap implementation: an HDR display (Rec.2100-PQ/HLG), or a
    /// view the display does not offer in Blender.
    NotAvailable {
        /// View transform name.
        view_transform: &'static str,
        /// Look name.
        look: String,
        /// Display name.
        display: &'static str,
    },
    /// A baked pipeline under `assets/luts/` failed to load (a build defect).
    Pipeline(String),
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
            ColorError::Pipeline(e) => write!(f, "corrupt colour-management data: {e}"),
        }
    }
}

impl std::error::Error for ColorError {}

/// A ready-to-apply display transform (Blender's display `CPUProcessor`).
#[derive(Clone, Debug)]
pub struct DisplayProcessor {
    scale: f32,
    exponent: f32,
    pipeline: &'static Pipeline,
}

impl DisplayProcessor {
    /// Builds the processor for `view` on `display`: the baked OCIO op pipeline of the
    /// view × look Blender applies (a look the view ignores, or a no-op look, renders as
    /// "None") on that display, between exposure and gamma.
    ///
    /// Errors with [`ColorError::NotAvailable`] on the HDR displays (Rec.2100-PQ/HLG) and for
    /// views the display does not offer; callers may fall back to Standard with a warning.
    pub fn new(view: &ViewSettings, display: &DisplaySettings) -> Result<Self, ColorError> {
        let look = crate::looks::effective_look(view.view_transform, &view.look)?;
        let pipeline = ocio::pipeline(
            display.display_device.name(),
            view.view_transform.name(),
            look,
        )
        .ok_or_else(|| ColorError::NotAvailable {
            view_transform: view.view_transform.name(),
            look: view.look.clone(),
            display: display.display_device.name(),
        })?
        .map_err(ColorError::Pipeline)?;
        let scale = if view.exposure == 0.0 {
            1.0
        } else {
            libm::powf(2.0, view.exposure)
        };
        let exponent = (1.0 / view.gamma.max(f32::EPSILON)).min(100.0);
        Ok(DisplayProcessor {
            scale,
            exponent,
            pipeline,
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
        let d = self.pipeline.apply(x);
        if self.exponent == 1.0 {
            d
        } else {
            d.map(|v| {
                if v > 0.0 {
                    ocio::sse_power(v, self.exponent)
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
    use crate::spaces::linear_to_srgb;

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
    fn every_view_and_look_is_available_on_every_sdr_display_that_offers_it() {
        for d in [
            DisplayDevice::Srgb,
            DisplayDevice::DisplayP3,
            DisplayDevice::Rec1886,
            DisplayDevice::Rec2020,
        ] {
            let ds = DisplaySettings { display_device: d };
            for vt in ViewTransform::ALL {
                // Blender 5.2.4's config: only sRGB offers Filmic, Filmic Log and Khronos PBR
                // Neutral.
                let offered = d == DisplayDevice::Srgb
                    || !matches!(
                        vt,
                        ViewTransform::Filmic
                            | ViewTransform::FilmicLog
                            | ViewTransform::KhronosPbrNeutral
                    );
                for look in crate::looks::looks_for(vt) {
                    let v = ViewSettings {
                        view_transform: vt,
                        look: look.to_owned(),
                        ..ViewSettings::default()
                    };
                    let p = DisplayProcessor::new(&v, &ds);
                    assert_eq!(p.is_ok(), offered, "{} {} {look}", d.name(), vt.name());
                }
            }
        }
    }

    #[test]
    fn processor_availability() {
        let d = DisplaySettings::default();
        let mut v = ViewSettings::default();
        assert!(DisplayProcessor::new(&v, &d).is_ok());
        v.look = "AgX - Punchy".into();
        assert!(DisplayProcessor::new(&v, &d).is_ok());
        // A look that does not apply to the view is ignored, as in Blender.
        v.look = "High Contrast".into();
        let ignored = DisplayProcessor::new(&v, &d).unwrap();
        v.look = "None".into();
        let none = DisplayProcessor::new(&v, &d).unwrap();
        assert_eq!(
            ignored.apply_rgb([0.3, 0.2, 0.1]),
            none.apply_rgb([0.3, 0.2, 0.1])
        );
        v.look = "Punchy".into();
        assert!(matches!(
            DisplayProcessor::new(&v, &d),
            Err(ColorError::UnknownLook(_))
        ));
        // The HDR displays are not baked.
        let pq = DisplaySettings {
            display_device: DisplayDevice::Rec2100Pq,
        };
        v.view_transform = ViewTransform::Standard;
        v.look = "None".into();
        let e = DisplayProcessor::new(&v, &pq).unwrap_err();
        assert_eq!(
            e.to_string(),
            "view transform 'Standard' with look 'None' on display 'Rec.2100-PQ' is not available in frap yet"
        );
    }

    #[test]
    fn standard_raw_exposure_gamma() {
        let d = DisplaySettings::default();
        let mut v = ViewSettings {
            view_transform: ViewTransform::Standard,
            ..ViewSettings::default()
        };
        let p = DisplayProcessor::new(&v, &d).unwrap();
        assert!((p.apply_rgb([0.18; 3])[0] - linear_to_srgb(0.18)).abs() < 1e-4);
        v.exposure = 1.0;
        v.gamma = 2.0;
        let p = DisplayProcessor::new(&v, &d).unwrap();
        let want = ocio::sse_power(linear_to_srgb(0.36), 0.5);
        assert!((p.apply_rgb([0.18; 3])[0] - want).abs() < 1e-4);
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
