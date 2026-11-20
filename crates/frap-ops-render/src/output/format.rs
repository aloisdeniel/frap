// SPDX-License-Identifier: GPL-3.0-or-later
//! `scene.render.image_settings` → encoder, and the film → file samples conversion (§19.4 step 3,
//! §19.5; WP 5.4). Every step follows what Blender 5.2.4 does to a render result written with
//! `render.render(write_still=True)` (GPL-2.0-or-later sources, verified against Blender's
//! files of one film in every format, `tests/golden/fixtures/render/output/film/`):
//!
//! 1. The render buffer is premultiplied scene-linear RGBA; frap's film is straight, so it is
//!    premultiplied first (`c * a`).
//! 2. `color_mode='BW'` (`RE_render_result_rect_to_ibuf` → `IMB_color_to_bw`): RGB becomes the
//!    Rec.709 luma of the premultiplied linear colour (`IMB_colormanagement_get_luminance`).
//!    OpenEXR (`image_exr_from_rgb_to_bw`) writes that luma as its single channel `V`.
//! 3. Not `RGBA` (`IMB_colormanagement_imbuf_for_write`: `IMB_alpha_under_color_float` over
//!    black): alpha becomes 1, the premultiplied colour stays.
//! 4. Display-referred formats (PNG, JPEG, WebP, Targa, BMP) get the view transform
//!    (`colormanage_display_buffer_process`, `apply_predivide`: for alpha other than 0 and 1
//!    the colour is divided by alpha, transformed, multiplied back). Linear formats (OpenEXR,
//!    Radiance HDR) keep the scene-linear samples.
//! 5. 8 bits (`IMB_buffer_byte_from_float` with predivide): `premul_to_straight`, then
//!    `render.dither_intensity` noise (`float_to_byte_dither_v4`) and
//!    `unit_float_to_uchar_clamp`. 16-bit PNG: OpenImageIO un-premultiplies the display float
//!    (`c / a`, `oiio:UnassociatedAlpha` 0) and rounds to 16 bits; a grey file is the weighted
//!    channel sum of the grey RGB (`oiio_write_prepare` → `ImageBufAlgo::channel_sum`).
//! 6. The writer: `color_mode` gives the channels (JPEG and HDR have no alpha; HDR stores grey
//!    as RGB), `TARGA` is run-length encoded and `TARGA_RAW` is not, JPEG qualities below 10 are
//!    90 (`BKE_image_format_to_imbuf`), WebP quality 100 is lossless, OpenEXR uses
//!    `exr_codec`, BMP / PNG / OpenEXR carry the render's pixels per metre
//!    (`BKE_scene_ppm_get`).
//!
//! Blender cannot write a one-channel WebP: the write fails after rendering with
//! `Render error (<strerror(0)>) cannot save: '<path>'`, reproduced by [`write_render`].

use std::path::Path;

use frap_color::display::{dither_random_value, unit_float_to_uchar_clamp};
use frap_color::spaces::luminance;
use frap_color::{
    ColorError, DisplayDevice, DisplayImage, DisplayProcessor, DisplaySettings, ViewSettings,
    ViewTransform,
};
use frap_imbuf::encode::{ExrCodec, WriteOptions};
use frap_imbuf::{DecodedImage, EncodeFormat, PixelData};
use frap_scene::generated::plain::{
    ColorManagedDisplaySettings, ColorManagedViewSettings, ImageFormatSettings, RenderSettings,
};

/// How to write a render result.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteSettings {
    /// Encoder and its options.
    pub encode: EncodeFormat,
    /// Output channels: 1 (`BW`), 3 (`RGB`), 4 (`RGBA`); 3 for `BW` Radiance HDR (grey RGB).
    pub channels: u8,
    /// Stores scene-linear floats (OpenEXR, HDR) instead of display-encoded values.
    pub linear: bool,
    /// `color_mode == 'BW'`.
    pub bw: bool,
    /// Writer options (Targa compression, OpenEXR codec, pixels per metre, sRGB chunks).
    pub options: WriteOptions,
    /// Blender's writer cannot write this combination (`WEBP` + `BW`): writing fails like
    /// Blender's, after rendering.
    pub write_fails: bool,
}

/// Why `image_settings` cannot be written (`NotImplementedError` in the operator).
pub fn unsupported(what: &str) -> String {
    format!("frap: render output {what} is not supported yet")
}

/// `BKE_scene_ppm_get`: pixels per metre from `ppm_factor / ppm_base`, lowered on the denser
/// axis of a non-square `pixel_aspect`. `None` when `ppm_base` is 0 (Blender ignores it).
pub fn render_ppm(r: &RenderSettings) -> Option<[f64; 2]> {
    if r.ppm_base == 0.0 {
        return None;
    }
    let (xasp, yasp) = (f64::from(r.pixel_aspect_x), f64::from(r.pixel_aspect_y));
    let (mut x, mut y) = (1.0, 1.0);
    if xasp < yasp {
        y = xasp / yasp;
    } else if xasp > yasp {
        x = yasp / xasp;
    }
    let ppm = f64::from(r.ppm_factor) / f64::from(r.ppm_base);
    Some([ppm * x, ppm * y])
}

/// [`write_settings`] for the render settings: the image settings plus the render's pixels per
/// metre.
pub fn render_write_settings(r: &RenderSettings) -> Result<WriteSettings, String> {
    let mut ws = write_settings(&r.image_settings)?;
    ws.options.ppm = render_ppm(r);
    Ok(ws)
}

/// Maps `image_settings` to an encoder (`Err`: a format frap does not write yet, message for
/// `NotImplementedError`). No pixels per metre: see [`render_write_settings`].
pub fn write_settings(s: &ImageFormatSettings) -> Result<WriteSettings, String> {
    if s.media_type != "IMAGE" {
        return Err(unsupported(&format!(
            "image_settings.media_type='{}'",
            s.media_type
        )));
    }
    let depth = s.color_depth.as_str();
    let quality = s.quality.clamp(0, 100) as u8;
    let mut options = WriteOptions::default();
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
                // `BKE_image_format_to_imbuf`: qualities below 10 are 90.
                quality: if s.quality < 10 { 90 } else { quality },
            },
            false,
        ),
        // `prepare_save_webp`: quality 100 is lossless.
        "WEBP" => (
            EncodeFormat::WebP {
                lossless: s.quality == 100,
                quality,
            },
            false,
        ),
        "OPEN_EXR" => {
            options.exr_codec =
                ExrCodec::from_identifier(&s.exr_codec, quality).ok_or_else(|| {
                    unsupported(&format!("image_settings.exr_codec='{}'", s.exr_codec))
                })?;
            options.exr_grey_channel = "V";
            options.exr_interop_id = Some("lin_rec709_scene");
            (
                EncodeFormat::OpenExr {
                    bit_depth: if depth == "16" { 16 } else { 32 },
                },
                true,
            )
        }
        "HDR" => (EncodeFormat::Hdr, true),
        "TARGA" => (EncodeFormat::Targa, false),
        "TARGA_RAW" => {
            options.targa_rle = false;
            (EncodeFormat::Targa, false)
        }
        "BMP" => (EncodeFormat::Bmp, false),
        other => {
            return Err(unsupported(&format!(
                "image_settings.file_format='{other}'"
            )));
        }
    };
    if linear && s.color_management == "OVERRIDE" {
        // `image_exr_from_scene_linear_to_output`: frap writes scene-linear Rec.709 only.
        let name = s.linear_colorspace_settings.name.as_str();
        if !matches!(name, "" | "Linear Rec.709") {
            return Err(unsupported(&format!(
                "image_settings.linear_colorspace_settings.name='{name}'"
            )));
        }
    }
    options.srgb = !linear;
    let bw = s.color_mode == "BW";
    let channels = match s.color_mode.as_str() {
        "BW" => 1,
        "RGB" => 3,
        _ => 4,
    };
    let channels = match encode {
        EncodeFormat::Jpeg { .. } if channels == 4 => 3,
        // Radiance stores RGB: grey is written as grey RGB.
        EncodeFormat::Hdr => 3,
        _ => channels,
    };
    let write_fails = bw && matches!(encode, EncodeFormat::WebP { .. });
    Ok(WriteSettings {
        encode,
        channels,
        linear,
        bw,
        options,
        write_fails,
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

/// Luminance weights of OpenImageIO's grey conversion (`IMB_colormanagement_get_luminance_
/// coefficients`, alpha weight 0).
const LUMA: [f32; 3] = frap_color::spaces::LUMA_REC709;

/// `ImageBufAlgo::channel_sum` of a grey RGB(A) pixel (alpha weight 0).
fn channel_sum(p: [f32; 4]) -> f32 {
    let mut sum = 0.0f32;
    sum += p[0] * LUMA[0];
    sum += p[1] * LUMA[1];
    sum += p[2] * LUMA[2];
    sum += p[3] * 0.0;
    sum
}

/// OpenImageIO's float → 16-bit conversion (`convert_type<float, uint16_t>`: round to nearest,
/// clamp).
fn oiio_float_to_u16(v: f32) -> u16 {
    let s = (v * 65535.0).round_ties_even();
    if s.is_nan() {
        0
    } else {
        s.clamp(0.0, 65535.0) as u16
    }
}

/// The samples Blender writes for a premultiplied scene-linear render buffer (top row first;
/// see the module documentation for the steps).
pub fn file_pixels_premultiplied(
    premul: &[[f32; 4]],
    width: u32,
    height: u32,
    ws: &WriteSettings,
    cm: &ColorManagement,
    dither_intensity: f32,
) -> Result<DecodedImage, String> {
    let n = premul.len();
    if n != width as usize * height as usize {
        return Err(format!(
            "{width}x{height} film has {n} pixels instead of {}",
            width as usize * height as usize
        ));
    }
    // OpenEXR BW is the luma of the render buffer, alpha dropped (`image_exr_from_rgb_to_bw`).
    if matches!(ws.encode, EncodeFormat::OpenExr { .. }) {
        let data: Vec<f32> = match ws.channels {
            1 => premul
                .iter()
                .map(|p| luminance([p[0], p[1], p[2]]))
                .collect(),
            3 => premul.iter().flat_map(|p| [p[0], p[1], p[2]]).collect(),
            _ => premul.iter().flatten().copied().collect(),
        };
        return DecodedImage::new(width, height, ws.channels, PixelData::F32(data))
            .map_err(|e| e.to_string());
    }
    // `IMB_color_to_bw`, then `IMB_alpha_under_color_float` over black.
    let under = ws.channels != 4;
    let buf: Vec<[f32; 4]> = premul
        .iter()
        .map(|&p| {
            let mut p = p;
            if ws.bw {
                let l = luminance([p[0], p[1], p[2]]);
                p = [l, l, l, p[3]];
            }
            if under {
                p[3] = 1.0;
            }
            p
        })
        .collect();
    if ws.linear {
        // Radiance HDR: RGB of the (grey) buffer.
        let data = buf.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
        return DecodedImage::new(width, height, 3, PixelData::F32(data))
            .map_err(|e| e.to_string());
    }
    // `apply_predivide`.
    let display: Vec<[f32; 4]> = buf
        .iter()
        .map(|&p| {
            let a = p[3];
            if a == 0.0 || a == 1.0 {
                cm.processor.apply_rgba(p)
            } else {
                let inv = 1.0 / a;
                let [r, g, b] = cm.processor.apply_rgb([p[0] * inv, p[1] * inv, p[2] * inv]);
                [r * a, g * a, b * a, a]
            }
        })
        .collect();
    let c = usize::from(ws.channels);
    let pixels = if let EncodeFormat::Png { bit_depth: 16, .. } = ws.encode {
        let mut out = Vec::with_capacity(n * c);
        for p in &display {
            match c {
                1 => out.push(oiio_float_to_u16(channel_sum(*p))),
                3 => out.extend(p[..3].iter().map(|&v| oiio_float_to_u16(v))),
                _ => {
                    // PNG output deassociates (`c / a` for alpha other than 0 and 1).
                    let a = p[3];
                    let straight = if a != 0.0 && a != 1.0 {
                        [p[0] / a, p[1] / a, p[2] / a, a]
                    } else {
                        *p
                    };
                    out.extend(straight.map(oiio_float_to_u16));
                }
            }
        }
        PixelData::U16(out)
    } else {
        let w = width as usize;
        let mut out = Vec::with_capacity(n * c);
        for (i, p) in display.iter().enumerate() {
            // `premul_to_straight_v4_v4` / `premul_float_to_straight_uchar`.
            let a = p[3];
            let s = if a == 0.0 || a == 1.0 {
                *p
            } else {
                let inv = 1.0 / a;
                [p[0] * inv, p[1] * inv, p[2] * inv, a]
            };
            let d = if dither_intensity != 0.0 {
                // Blender's rows count from the bottom.
                let (x, y) = ((i % w) as i32, (height as usize - 1 - i / w) as i32);
                dither_random_value(x, y) * 0.0033 * dither_intensity
            } else {
                0.0
            };
            let q = |v: f32| unit_float_to_uchar_clamp(if d != 0.0 { d + v } else { v });
            let bytes = [q(s[0]), q(s[1]), q(s[2]), unit_float_to_uchar_clamp(s[3])];
            // Grey files: OpenImageIO's channel sum of equal bytes gives the byte back.
            out.extend_from_slice(&bytes[..c]);
        }
        PixelData::U8(out)
    };
    DecodedImage::new(width, height, ws.channels, pixels).map_err(|e| e.to_string())
}

/// The samples to encode for a straight-alpha scene-linear film (premultiplied first, as
/// Blender's render buffer is).
pub fn file_pixels(
    rgba: &[[f32; 4]],
    width: u32,
    height: u32,
    ws: &WriteSettings,
    cm: &ColorManagement,
    dither_intensity: f32,
) -> Result<DecodedImage, String> {
    let premul: Vec<[f32; 4]> = rgba
        .iter()
        .map(|p| [p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
        .collect();
    file_pixels_premultiplied(&premul, width, height, ws, cm, dither_intensity)
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
    frap_imbuf::encode::encode_with(&img, &ws.encode, &ws.options).map_err(|e| e.to_string())
}

/// Blender's report when a render cannot be saved (`BKE_image_render_write`): `strerror(errno)`
/// with `errno` 0 for writer failures (`imb_oiio_write` clears it).
pub fn cannot_save(path: &Path) -> String {
    let e = std::io::Error::from_raw_os_error(0);
    format!(
        "Render error ({}) cannot save: '{}'",
        frap_rna::error::strerror(&e),
        path.display()
    )
}

/// Encodes the film and writes it to `path` ([`crate::output::write_file`]), failing as Blender
/// does for combinations its writers refuse (`Err`: the message for `RuntimeError: Error: …`).
pub fn write_render(
    path: &Path,
    rgba: &[[f32; 4]],
    width: u32,
    height: u32,
    ws: &WriteSettings,
    cm: &ColorManagement,
    dither_intensity: f32,
) -> Result<(), String> {
    if ws.write_fails {
        return Err(cannot_save(path));
    }
    let bytes = encode(rgba, width, height, ws, cm, dither_intensity)?;
    crate::output::write_file(path, &bytes)
}

#[cfg(test)]
mod tests;
