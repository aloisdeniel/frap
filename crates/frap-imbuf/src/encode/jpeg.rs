// SPDX-License-Identifier: GPL-3.0-or-later
//! JPEG (`image` crate; WP 5.4).

use super::raw8::color_type_8;
use crate::codec::{DecodedImage, ImageError};

pub(crate) fn encode_jpeg(image: &DecodedImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    use image::ImageEncoder;
    // JPEG has no alpha: BW+alpha → BW, RGBA → RGB (as Blender, which offers BW/RGB only).
    let img = match image.channels {
        2 => image.with_channels(1)?,
        4 => image.with_channels(3)?,
        _ => image.clone(),
    };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
        .write_image(
            &img.to_u8(),
            img.width,
            img.height,
            color_type_8(img.channels),
        )
        .map_err(|e| ImageError::Encode(format!("JPEG: {e}")))?;
    Ok(out)
}
