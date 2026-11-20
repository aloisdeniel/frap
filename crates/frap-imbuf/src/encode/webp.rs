// SPDX-License-Identifier: GPL-3.0-or-later
//! WebP: lossless (`image` crate), lossy behind the `webp-lossy` feature (WP 5.4).
//!
//! Blender 5.2.4 (`format_webp.cc` → OIIO / libwebp) promotes grey + alpha to RGBA, writes
//! quality 100 lossless and anything else lossy at that quality (method 4). Blender cannot write
//! one-channel WebP (`render.render` with `color_mode='BW'` fails to save); this encoder still
//! accepts grey images for other callers.

use super::raw8::color_type_8;
use crate::codec::{DecodedImage, ImageError};

pub(crate) fn encode_webp(
    image: &DecodedImage,
    lossless: bool,
    quality: u8,
) -> Result<Vec<u8>, ImageError> {
    use image::ImageEncoder;
    let promoted;
    let image = if image.channels == 2 {
        promoted = image.with_channels(4)?;
        &promoted
    } else {
        image
    };
    if !lossless {
        return encode_webp_lossy(image, quality);
    }
    let mut out = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut out)
        .write_image(
            &image.to_u8(),
            image.width,
            image.height,
            color_type_8(image.channels),
        )
        .map_err(|e| ImageError::Encode(format!("WEBP: {e}")))?;
    Ok(out)
}

#[cfg(feature = "webp-lossy")]
fn encode_webp_lossy(image: &DecodedImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    let img = match image.channels {
        1 => image.with_channels(3)?,
        2 => image.with_channels(4)?,
        _ => image.clone(),
    };
    let data = img.to_u8();
    let enc = if img.channels == 4 {
        webp::Encoder::from_rgba(&data, img.width, img.height)
    } else {
        webp::Encoder::from_rgb(&data, img.width, img.height)
    };
    let mem = enc
        .encode_simple(false, f32::from(quality.min(100)))
        .map_err(|e| ImageError::Encode(format!("WEBP: {e:?}")))?;
    Ok(mem.to_vec())
}

#[cfg(not(feature = "webp-lossy"))]
fn encode_webp_lossy(_image: &DecodedImage, _quality: u8) -> Result<Vec<u8>, ImageError> {
    Err(ImageError::Unsupported(
        "lossy WebP encoding needs frap built with the `webp-lossy` feature (lossless WebP \
         is always available)"
            .into(),
    ))
}
