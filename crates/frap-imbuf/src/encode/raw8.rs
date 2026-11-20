// SPDX-License-Identifier: GPL-3.0-or-later
//! 8-bit formats through the `image` crate: Targa and BMP, and the channel layout helper the
//! other `image` encoders share (WP 5.4).

use crate::codec::{DecodedImage, ImageError};

pub(crate) fn color_type_8(channels: u8) -> image::ExtendedColorType {
    match channels {
        1 => image::ExtendedColorType::L8,
        2 => image::ExtendedColorType::La8,
        3 => image::ExtendedColorType::Rgb8,
        _ => image::ExtendedColorType::Rgba8,
    }
}

pub(crate) fn encode_8bit(
    image: &DecodedImage,
    format: image::ImageFormat,
) -> Result<Vec<u8>, ImageError> {
    use image::ImageEncoder;
    let data = image.to_u8();
    let mut out = Vec::new();
    let ct = color_type_8(image.channels);
    let res = match format {
        image::ImageFormat::Tga => image::codecs::tga::TgaEncoder::new(&mut out).write_image(
            &data,
            image.width,
            image.height,
            ct,
        ),
        _ => image::codecs::bmp::BmpEncoder::new(&mut out).write_image(
            &data,
            image.width,
            image.height,
            ct,
        ),
    };
    res.map_err(|e| ImageError::Encode(format!("{format:?}: {e}")))?;
    Ok(out)
}
