// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.save_render(filepath, scene=None, quality=0)` (`rna_Image_save_render` →
//! `BKE_image_save` with `save_as_render` and `save_copy`, `image_save.cc` and
//! `IMB_colormanagement_imbuf_for_write`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Writes the image with the scene's render output settings (`render.image_settings`: format,
//! colour mode, depth; `quality` overrides theirs when not 0) and, for display formats, the
//! scene's view transform (`view_settings`/`display_settings`, or the image settings' own with
//! `color_management='OVERRIDE'`); OpenEXR/HDR get scene-linear floats. The image is not
//! changed. Verified on 5.2.4 (AgX and Standard, byte and float images): byte pixels are
//! linearised from the image's colour space before the view transform, float pixels are
//! unpremultiplied; no dither. Errors: `Image 'X' does not have any image data`, and
//! `Could not write image: …` + `Image 'X' could not be saved to '<filepath>'`.
//!
//! frap gap: the `Render Result` of `render.render()` is kept outside the datablocks, so its
//! `save_render()` reports no image data; render with `render.render(write_still=True)` and
//! `scene.render.filepath` instead.

use frap_color::{DisplayDevice, DisplayProcessor, DisplaySettings, ViewSettings, ViewTransform};
use frap_imbuf::{DecodedImage, EncodeFormat, PixelData};
use frap_rna::{IdRef, RnaError, RnaPtr, SceneId, Value, rna_func};
use frap_scene::id::ImBufData;
use frap_scene::id::PathBase;
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_id, arg_int, arg_str, internal, not_implemented};
use super::image_save::write::{Reports, alpha_under_black_byte, image, write_file};
use super::image_save::{encoder, linear_format};

/// `srgb_to_linearrgb`
fn srgb_to_linear(c: f32) -> f32 {
    if c < 0.04045 {
        return if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) };
    }
    ((c + 0.055) * (1.0 / 1.055)).powf(2.4)
}

#[rna_func(srna = "Image", func = "save_render")]
fn image_save_render(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let filepath = arg_str(args, "filepath").to_owned();
    let quality = arg_int(args, "quality", 0);
    let scene_ref: IdRef<SceneId> = arg_id(args, "scene")
        .and_then(IdRef::from_any)
        .unwrap_or(h.win().scene);
    let base = PathBase::of(h);
    let im = image(h, this)?;
    let name = im.header.name.clone();
    let mut rep = Reports::default();
    let Some(buf) = im.acquire(&base) else {
        rep.error(format!("Image '{name}' does not have any image data"));
        rep.finish(h)?;
        return Ok(Value::None);
    };
    let is_data = im.is_data_colorspace();
    let scene = h
        .main()
        .scenes
        .get(scene_ref)
        .ok_or_else(|| internal("the scene was removed"))?;
    let s = &scene.render.image_settings;
    let q = if quality != 0 {
        quality.clamp(0, 100) as u8
    } else {
        s.quality.clamp(0, 100) as u8
    };
    let mut enc = encoder(&s.file_format, false, q).map_err(not_implemented)?;
    if let EncodeFormat::Png {
        bit_depth,
        compression,
    } = &mut enc
    {
        *bit_depth = if s.color_depth == "16" { 16 } else { 8 };
        *compression = s.compression.clamp(0, 100) as u8;
    }
    if let EncodeFormat::OpenExr { bit_depth } = &mut enc {
        *bit_depth = if s.color_depth == "16" { 16 } else { 32 };
    }
    let channels: u8 = match (s.color_mode.as_str(), &enc) {
        ("BW", _) => 1,
        (_, EncodeFormat::Jpeg { .. } | EncodeFormat::Hdr) => 3,
        ("RGB", _) => 3,
        _ => 4,
    };
    let (view, display) = if s.color_management == "OVERRIDE" {
        (&s.view_settings, &s.display_settings)
    } else {
        (&scene.view_settings, &scene.display_settings)
    };
    let mut warnings = Vec::new();
    let display_device = DisplayDevice::from_name(&display.display_device).unwrap_or_else(|| {
        warnings.push(format!(
            "frap: display device '{}' is not available in frap; using sRGB",
            display.display_device
        ));
        DisplayDevice::Srgb
    });
    let view_transform = ViewTransform::from_name(&view.view_transform).unwrap_or_else(|| {
        warnings.push(format!(
            "frap: view transform '{}' is not available in frap; saved with Standard",
            view.view_transform
        ));
        ViewTransform::Standard
    });
    let vs = ViewSettings {
        view_transform,
        look: view.look.to_string(),
        exposure: view.exposure,
        gamma: view.gamma,
    };
    let ds = DisplaySettings { display_device };
    let processor = DisplayProcessor::new(&vs, &ds).or_else(|e| {
        warnings.push(format!("frap: {e}; saved with Standard, look None"));
        let std = ViewSettings {
            view_transform: ViewTransform::Standard,
            look: "None".into(),
            ..vs.clone()
        };
        DisplayProcessor::new(&std, &ds)
            .or_else(|_| DisplayProcessor::new(&std, &DisplaySettings::default()))
    });
    let processor = processor.map_err(|e| internal(e.to_string()))?;
    // Scene-linear, straight pixels, top row first.
    let row = buf.x as usize * 4;
    let under_black = channels != 4;
    // A colour mode without alpha composites over black first (`IMB_alpha_under_color_*`, on
    // the stored values: sRGB bytes, premultiplied floats).
    let linear: Vec<[f32; 4]> = match &buf.data {
        ImBufData::Byte(b) => frap_scene::id::ImBuf::flip_rows(b, row)
            .chunks_exact(4)
            .map(|p| {
                frap_math::budget::check();
                let mut p = [p[0], p[1], p[2], p[3]];
                if under_black {
                    alpha_under_black_byte(&mut p);
                }
                let c = |v: u8| {
                    let v = f32::from(v) / 255.0;
                    if is_data { v } else { srgb_to_linear(v) }
                };
                [c(p[0]), c(p[1]), c(p[2]), f32::from(p[3]) / 255.0]
            })
            .collect(),
        ImBufData::Float(f) => frap_scene::id::ImBuf::flip_rows(f, row)
            .chunks_exact(4)
            .map(|p| {
                if under_black {
                    return [p[0], p[1], p[2], 1.0];
                }
                let a = p[3];
                let un = if a == 0.0 || a == 1.0 { 1.0 } else { 1.0 / a };
                [p[0] * un, p[1] * un, p[2] * un, a]
            })
            .collect(),
    };
    let pixels = if linear_format(&enc) {
        PixelData::F32(linear.iter().flatten().copied().collect())
    } else {
        let disp: Vec<[f32; 4]> = linear
            .iter()
            .map(|p| {
                // `--stmt-timeout` (ARCHITECTURE §12): the display transform is the slow step.
                frap_math::budget::check();
                if is_data {
                    *p
                } else {
                    processor.apply_rgba(*p)
                }
            })
            .collect();
        match enc {
            EncodeFormat::Png { bit_depth: 16, .. } => PixelData::U16(
                disp.iter()
                    .flatten()
                    .map(|&v| frap_color::display::unit_float_to_ushort_clamp(v))
                    .collect(),
            ),
            _ => PixelData::U8(
                disp.iter()
                    .flatten()
                    .map(|&v| frap_color::display::unit_float_to_uchar_clamp(v))
                    .collect(),
            ),
        }
    };
    let img = DecodedImage::new(buf.x, buf.y, 4, pixels)
        .and_then(|i| i.with_channels(channels))
        .map_err(|e| internal(e.to_string()))?;
    let bytes = frap_imbuf::encode(&img, &enc).map_err(|e| not_implemented(e.to_string()))?;
    for w in warnings {
        rep.warning(w);
    }
    if let Err(e) = write_file(&base, &base.abs(&filepath), &bytes) {
        rep.error(format!("Could not write image: {e}"));
        rep.error(format!("Image '{name}' could not be saved to '{filepath}'"));
    }
    rep.finish(h)?;
    Ok(Value::None)
}
