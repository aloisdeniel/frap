// SPDX-License-Identifier: GPL-3.0-or-later
//! `scene.render.image_settings` → encoder, and the film → file pixels conversion (§19.4 step 3,
//! §19.5).
//!
//! As Blender's `IMB_colormanagement_imbuf_for_write`: display-referred formats (PNG, JPEG,
//! WebP, Targa, BMP) get the view transform (scene `view_settings`/`display_settings`, or the
//! image settings' own when `color_management == 'OVERRIDE'`), then 8-bit quantisation with
//! `render.dither_intensity` (16-bit PNG: no dither); float formats (OpenEXR, Radiance HDR) keep
//! the scene-linear film. `color_mode` `BW` is Rec.709 luma of the encoded values
//! (`IMB_color_to_bw`), `RGB` drops alpha.

use frap_color::{
    ColorError, DisplayDevice, DisplayImage, DisplayProcessor, DisplaySettings, ViewSettings,
    ViewTransform,
};
use frap_imbuf::{DecodedImage, EncodeFormat, PixelData};
use frap_scene::generated::plain::{
    ColorManagedDisplaySettings, ColorManagedViewSettings, ImageFormatSettings,
};

/// How to write a render result.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteSettings {
    /// Encoder and its options.
    pub encode: EncodeFormat,
    /// Output channels: 1 (`BW`), 3 (`RGB`), 4 (`RGBA`).
    pub channels: u8,
    /// Stores scene-linear floats (OpenEXR, HDR) instead of display-encoded values.
    pub linear: bool,
}

/// Why `image_settings` cannot be written (`NotImplementedError` in the operator).
pub fn unsupported(what: &str) -> String {
    format!("frap: render output {what} is not supported yet")
}

/// Maps `image_settings` to an encoder (`Err`: a format frap does not write yet, message for
/// `NotImplementedError`).
pub fn write_settings(s: &ImageFormatSettings) -> Result<WriteSettings, String> {
    if s.media_type != "IMAGE" {
        return Err(unsupported(&format!(
            "image_settings.media_type='{}'",
            s.media_type
        )));
    }
    let depth = s.color_depth.as_str();
    let (encode, linear) = match s.file_format.as_str() {
        "PNG" => (
            EncodeFormat::Png {
                bit_depth: if depth == "16" { 16 } else { 8 },
                compression: s.compression.clamp(0, 100) as u8,
            },
            false,
        ),
        "JPEG" => (
            EncodeFormat::Jpeg {
                quality: s.quality.clamp(0, 100) as u8,
            },
            false,
        ),
        // Blender's WebP writer is lossless at quality 100 (`imb_savewebp`).
        "WEBP" => (
            EncodeFormat::WebP {
                lossless: s.quality >= 100,
                quality: s.quality.clamp(0, 100) as u8,
            },
            false,
        ),
        "OPEN_EXR" => (
            EncodeFormat::OpenExr {
                bit_depth: if depth == "16" { 16 } else { 32 },
            },
            true,
        ),
        "HDR" => (EncodeFormat::Hdr, true),
        "TARGA" | "TARGA_RAW" => (EncodeFormat::Targa, false),
        "BMP" => (EncodeFormat::Bmp, false),
        other => {
            return Err(unsupported(&format!(
                "image_settings.file_format='{other}'"
            )));
        }
    };
    let channels = match s.color_mode.as_str() {
        "BW" => 1,
        "RGB" => 3,
        _ => 4,
    };
    // Formats without alpha.
    let channels = match encode {
        EncodeFormat::Jpeg { .. } | EncodeFormat::Hdr if channels == 4 => 3,
        _ => channels,
    };
    Ok(WriteSettings {
        encode,
        channels,
        linear,
    })
}

/// A colour-management setup ready to apply, with the WARNINGs of any fallback.
#[derive(Clone, Debug)]
pub struct ColorManagement {
    /// The processor (view transform, look, exposure, gamma, display).
    pub processor: DisplayProcessor,
    /// Fallback reports (`frap: …`).
    pub warnings: Vec<String>,
}

/// `view_settings` + `display_settings` → a processor. A combination frap cannot reproduce
/// yet falls back to Standard with a WARNING (decision log W1-13); curve mapping and white
/// balance are not applied (WARNING).
pub fn color_management(
    view: &ColorManagedViewSettings,
    display: &ColorManagedDisplaySettings,
) -> ColorManagement {
    let mut warnings = Vec::new();
    let display_device = DisplayDevice::from_name(&display.display_device).unwrap_or_else(|| {
        warnings.push(format!(
            "frap: display device '{}' is not available in frap; using sRGB",
            display.display_device
        ));
        DisplayDevice::Srgb
    });
    let display = DisplaySettings { display_device };
    let view_transform = ViewTransform::from_name(&view.view_transform).unwrap_or_else(|| {
        warnings.push(format!(
            "frap: view transform '{}' is not available in frap; rendered with Standard",
            view.view_transform
        ));
        ViewTransform::Standard
    });
    let mut vs = ViewSettings {
        view_transform,
        look: view.look.to_string(),
        exposure: view.exposure,
        gamma: view.gamma,
    };
    if view.use_curve_mapping {
        warnings.push("frap: view_settings.use_curve_mapping is not applied yet".into());
    }
    if view.use_white_balance {
        warnings.push("frap: view_settings.use_white_balance is not applied yet".into());
    }
    let processor = match DisplayProcessor::new(&vs, &display) {
        Ok(p) => p,
        Err(e) => {
            match &e {
                ColorError::NotAvailable { .. } => {
                    warnings.push(format!("frap: {e}; rendered with Standard"))
                }
                _ => warnings.push(format!("frap: {e}; rendered with Standard, look None")),
            }
            vs.view_transform = ViewTransform::Standard;
            vs.look = "None".into();
            let srgb = DisplaySettings::default();
            DisplayProcessor::new(&vs, &display)
                .or_else(|_| DisplayProcessor::new(&vs, &srgb))
                .expect("invariant: Standard on sRGB is analytic")
        }
    };
    ColorManagement {
        processor,
        warnings,
    }
}

/// Applies `cm` to a scene-linear straight-alpha film (top row first).
pub fn display_image(
    rgba: &[[f32; 4]],
    width: u32,
    height: u32,
    cm: &ColorManagement,
) -> DisplayImage {
    DisplayImage {
        width,
        height,
        rgba: rgba.iter().map(|p| cm.processor.apply_rgba(*p)).collect(),
    }
}

/// The pixels to encode: display-referred (8 or 16 bit) or scene-linear floats, converted to
/// `ws.channels`.
pub fn file_pixels(
    rgba: &[[f32; 4]],
    width: u32,
    height: u32,
    ws: &WriteSettings,
    cm: &ColorManagement,
    dither_intensity: f32,
) -> Result<DecodedImage, String> {
    let pixels = if ws.linear {
        PixelData::F32(rgba.iter().flatten().copied().collect())
    } else {
        let d = display_image(rgba, width, height, cm);
        match ws.encode {
            EncodeFormat::Png { bit_depth: 16, .. } => PixelData::U16(d.to_rgba16()),
            _ => PixelData::U8(d.to_rgba8(dither_intensity)),
        }
    };
    let img = DecodedImage::new(width, height, 4, pixels).map_err(|e| e.to_string())?;
    img.with_channels(ws.channels).map_err(|e| e.to_string())
}

/// Encodes the film for `ws` (`Err`: encoder message).
pub fn encode(
    rgba: &[[f32; 4]],
    width: u32,
    height: u32,
    ws: &WriteSettings,
    cm: &ColorManagement,
    dither_intensity: f32,
) -> Result<Vec<u8>, String> {
    let img = file_pixels(rgba, width, height, ws, cm, dither_intensity)?;
    frap_imbuf::encode(&img, &ws.encode).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(fmt: &str, mode: &str, depth: &str) -> ImageFormatSettings {
        ImageFormatSettings {
            file_format: fmt.into(),
            color_mode: mode.into(),
            color_depth: depth.into(),
            ..ImageFormatSettings::default()
        }
    }

    #[test]
    fn factory_png_rgba8() {
        let ws = write_settings(&ImageFormatSettings::default()).unwrap();
        assert_eq!(
            ws,
            WriteSettings {
                encode: EncodeFormat::Png {
                    bit_depth: 8,
                    compression: 15
                },
                channels: 4,
                linear: false
            }
        );
    }

    #[test]
    fn formats() {
        let w = |f, m, d| write_settings(&settings(f, m, d)).unwrap();
        assert_eq!(
            w("PNG", "RGB", "16").encode,
            EncodeFormat::Png {
                bit_depth: 16,
                compression: 15
            }
        );
        assert_eq!(w("PNG", "BW", "8").channels, 1);
        assert_eq!(w("PNG", "RGB", "8").channels, 3);
        assert_eq!(
            w("JPEG", "RGB", "8").encode,
            EncodeFormat::Jpeg { quality: 90 }
        );
        assert_eq!(w("JPEG", "RGBA", "8").channels, 3);
        let exr = w("OPEN_EXR", "RGBA", "16");
        assert_eq!(exr.encode, EncodeFormat::OpenExr { bit_depth: 16 });
        assert!(exr.linear);
        assert_eq!(
            w("OPEN_EXR", "RGBA", "32").encode,
            EncodeFormat::OpenExr { bit_depth: 32 }
        );
        assert!(w("HDR", "RGB", "32").linear);
        assert_eq!(w("HDR", "RGBA", "32").channels, 3);
        assert_eq!(w("TARGA", "RGBA", "8").encode, EncodeFormat::Targa);
        assert_eq!(w("BMP", "RGB", "8").encode, EncodeFormat::Bmp);
        let mut s = settings("WEBP", "RGBA", "8");
        s.quality = 100;
        assert_eq!(
            write_settings(&s).unwrap().encode,
            EncodeFormat::WebP {
                lossless: true,
                quality: 100
            }
        );
        assert_eq!(
            write_settings(&settings("TIFF", "RGBA", "8")).unwrap_err(),
            "frap: render output image_settings.file_format='TIFF' is not supported yet"
        );
        let video = ImageFormatSettings {
            media_type: "VIDEO".into(),
            ..ImageFormatSettings::default()
        };
        assert!(write_settings(&video).is_err());
    }

    fn standard() -> ColorManagement {
        let view = ColorManagedViewSettings {
            view_transform: "Standard".into(),
            ..ColorManagedViewSettings::default()
        };
        color_management(&view, &ColorManagedDisplaySettings::default())
    }

    #[test]
    fn standard_png_pixels() {
        let cm = standard();
        assert!(cm.warnings.is_empty());
        let ws = write_settings(&ImageFormatSettings::default()).unwrap();
        // Linear 0.25 → sRGB 0.5371 → 137.
        let img = file_pixels(&[[0.25, 0.0, 1.0, 0.5]], 1, 1, &ws, &cm, 0.0).unwrap();
        assert_eq!(img.pixels, PixelData::U8(vec![137, 0, 255, 128]));
        let bw = write_settings(&settings("PNG", "BW", "8")).unwrap();
        let img = file_pixels(&[[1.0, 1.0, 1.0, 1.0]], 1, 1, &bw, &cm, 0.0).unwrap();
        assert_eq!((img.channels, img.pixels), (1, PixelData::U8(vec![255])));
        let exr = write_settings(&settings("OPEN_EXR", "RGB", "32")).unwrap();
        let img = file_pixels(&[[2.0, 0.5, 0.25, 0.5]], 1, 1, &exr, &cm, 0.0).unwrap();
        assert_eq!(
            (img.channels, img.pixels),
            (3, PixelData::F32(vec![2.0, 0.5, 0.25]))
        );
    }

    #[test]
    fn factory_agx_has_no_warning() {
        let cm = color_management(
            &ColorManagedViewSettings::default(),
            &ColorManagedDisplaySettings::default(),
        );
        assert!(cm.warnings.is_empty(), "{:?}", cm.warnings);
    }

    #[test]
    fn fallbacks_warn() {
        let view = ColorManagedViewSettings {
            view_transform: "Nope".into(),
            use_curve_mapping: true,
            ..ColorManagedViewSettings::default()
        };
        let cm = color_management(&view, &ColorManagedDisplaySettings::default());
        assert_eq!(cm.warnings.len(), 2, "{:?}", cm.warnings);
        assert!(cm.warnings[0].contains("'Nope'"));
    }

    #[test]
    fn encodes_png() {
        let ws = write_settings(&ImageFormatSettings::default()).unwrap();
        let bytes = encode(&[[0.5, 0.5, 0.5, 1.0]; 4], 2, 2, &ws, &standard(), 1.0).unwrap();
        let back = frap_imbuf::decode(&bytes).unwrap();
        assert_eq!((back.width, back.height, back.channels), (2, 2, 4));
    }
}
