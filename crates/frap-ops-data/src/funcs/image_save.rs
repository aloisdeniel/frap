// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.save(filepath='', quality=0, save_copy=False)` (`rna_Image_save` →
//! `BKE_image_save`, `image_save.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Writes the image's pixels in its own `file_format` to `filepath` (the image's own path when
//! empty). Verified on 5.2.4:
//! - the image's `filepath` is **not** changed (the RNA function leaves `do_newpath` off);
//!   without a path, or with a path that has no folder (`'x.png'`, Blender cannot create the
//!   folder `''`), it fails with `Error: Could not write image: No such file or directory` and
//!   `Error: Image 'X' could not be saved to '<image filepath>'`;
//! - the format comes from the buffer: `file_format` (a `.exr` name still gets PNG data unless
//!   `file_format = 'OPEN_EXR'`), 8 bits for a byte buffer, 16-bit PNG for a float buffer; the
//!   colour mode is the buffer's (24 → RGB, 32 → RGBA), and a modified RGBA buffer whose alpha
//!   is all opaque is written as RGB (`BKE_imbuf_alpha_test`);
//! - pixels are encoded to the image's colour space, `sRGB` for a generated image (so a float
//!   generated image is written sRGB-encoded, also to OpenEXR); a float buffer's premultiplied
//!   colours are converted first and divided by alpha afterwards for a 16-bit PNG, divided
//!   first for 8-bit formats, and written premultiplied to OpenEXR (`write::Unpremul`);
//! - afterwards (unless `save_copy`): not dirty, a GENERATED image becomes FILE (type
//!   `IMAGE`), `file_format` is the written format, and when the colour space changed (a float
//!   generated image becomes `sRGB`) the buffer is freed, which leaves an image without a path
//!   without data (`size` `(0, 0)`, `file_format` `TARGA` as for any image without a buffer);
//! - an image without data: `Error: Image 'X' does not have any image data`.

pub(crate) mod write;

use std::sync::Arc;

use frap_imbuf::EncodeFormat;
use frap_rna::{IdRef, ImageId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::PathBase;
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use self::write::{
    Encoding, Reports, Unpremul, alpha_test, image_ref, mode_channels, to_decoded, write_file,
};
use super::_common::{arg_bool, arg_int, arg_str, not_implemented};

/// The encoder of a `file_format` (`Err`: a format frap does not write).
pub(crate) fn encoder(file_format: &str, float: bool, quality: u8) -> Result<EncodeFormat, String> {
    Ok(match file_format {
        "PNG" => EncodeFormat::Png {
            bit_depth: if float { 16 } else { 8 },
            compression: 15,
        },
        "JPEG" => EncodeFormat::Jpeg { quality },
        "WEBP" => EncodeFormat::WebP {
            lossless: quality >= 100,
            quality,
        },
        "OPEN_EXR" => EncodeFormat::OpenExr {
            bit_depth: if float { 32 } else { 16 },
        },
        "HDR" => EncodeFormat::Hdr,
        "TARGA" | "TARGA_RAW" => EncodeFormat::Targa,
        "BMP" => EncodeFormat::Bmp,
        other => {
            return Err(format!(
                "frap: writing file_format '{other}' is not supported yet"
            ));
        }
    })
}

/// True for formats that store scene-linear floats (`BKE_imtype_requires_linear_float`).
pub(crate) fn linear_format(e: &EncodeFormat) -> bool {
    matches!(e, EncodeFormat::OpenExr { .. } | EncodeFormat::Hdr)
}

/// `BKE_image_save` for `Image.save()`; errors are added to `rep`.
pub(crate) fn save(
    h: &mut dyn Host,
    r: IdRef<ImageId>,
    filepath: &str,
    quality: i64,
    save_copy: bool,
    rep: &mut Reports,
) -> Result<(), RnaError> {
    let base = PathBase::of(h);
    let main = h.main();
    let im = main
        .images
        .get(r)
        .ok_or_else(|| super::_common::internal("the image was removed"))?;
    let name = im.header.name.clone();
    let Some(mut buf) = im.acquire(&base) else {
        rep.error(format!("Image '{name}' does not have any image data"));
        return Ok(());
    };
    // `BKE_image_save_options_init`: the generated image's colour space is the byte default.
    let to_colorspace: SmolStr = if im.source == "GENERATED" && !im.is_data_colorspace() {
        "sRGB".into()
    } else {
        im.colorspace_settings.name.clone()
    };
    let quality = if quality != 0 {
        quality.clamp(0, 100) as u8
    } else {
        90
    };
    let file_format = im.file_format.clone();
    let enc = encoder(&file_format, buf.is_float(), quality).map_err(not_implemented)?;
    let path = base.abs(if filepath.is_empty() {
        &im.filepath
    } else {
        filepath
    });
    let image_filepath = im.filepath.clone();
    // Colour mode: a modified RGBA buffer is tested for alpha.
    let mut channels = mode_channels(buf.planes);
    if channels == 4 && buf.dirty && !alpha_test(&buf) {
        channels = 3;
    }
    let is_data = im.is_data_colorspace();
    let encoding = if !buf.is_float()
        || is_data
        || frap_scene::id::imbuf::to_linear_curve(&to_colorspace).is_none()
    {
        Encoding::AsIs
    } else {
        Encoding::LinearToSrgb
    };
    let unpremul = match &enc {
        _ if !buf.is_float() || linear_format(&enc) => Unpremul::Never,
        // `prepare_save_png`: a float buffer that was converted (its colour space is set) or
        // is data is unassociated by OpenImageIO; one left scene linear is written as is.
        EncodeFormat::Png { bit_depth: 16, .. } => {
            if encoding == Encoding::LinearToSrgb || is_data {
                Unpremul::After
            } else {
                Unpremul::Never
            }
        }
        // `IMB_byte_from_float`: `predivide` unless the channels are packed.
        _ if im.source != "GENERATED" && (is_data || im.alpha_mode == "CHANNEL_PACKED") => {
            Unpremul::Never
        }
        _ => Unpremul::Before,
    };
    // `IMB_colormanagement_imbuf_for_write`: a colour mode without alpha composites over black.
    let img = to_decoded(&buf, channels, encoding, unpremul, channels != 4);
    let bytes = frap_imbuf::encode(&img, &enc).map_err(|e| not_implemented(e.to_string()))?;
    if let Err(e) = write_file(&base, &path, &bytes) {
        rep.error(format!("Could not write image: {e}"));
        rep.error(format!(
            "Image '{name}' could not be saved to '{image_filepath}'"
        ));
        return Ok(());
    }
    if save_copy {
        return Ok(());
    }
    let (main, log) = h.main_mut();
    if let Some(im) = main.images.get_mut(r, log) {
        if matches!(im.source.as_str(), "GENERATED" | "VIEWER") {
            im.source = SmolStr::new_static("FILE");
        }
        let ftype = SmolStr::new_static(enc.file_format().identifier());
        im.file_format = ftype.clone();
        if !to_colorspace.is_empty() && im.colorspace_settings.name != to_colorspace {
            // `IMA_SIGNAL_COLORMANAGE`: the buffer is freed; `file_format` is then the format
            // of the buffer loaded again from the image's path, `TARGA` (`IMB_FTYPE_NONE`)
            // without one.
            im.colorspace_settings.name = to_colorspace;
            im.free_buffers();
            im.file_format = im
                .info(&base)
                .map_or(SmolStr::new_static("TARGA"), |i| i.ftype);
        } else {
            buf.dirty = false;
            buf.ftype = ftype;
            im.buffer = Some(Arc::new(buf));
        }
    }
    Ok(())
}

#[rna_func(srna = "Image", func = "save")]
fn image_save(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let r = image_ref(this)?;
    let filepath = arg_str(args, "filepath").to_owned();
    let quality = arg_int(args, "quality", 0);
    let save_copy = arg_bool(args, "save_copy", false);
    let mut rep = Reports::default();
    save(h, r, &filepath, quality, save_copy, &mut rep)?;
    rep.finish(h)?;
    Ok(Value::None)
}
