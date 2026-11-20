// SPDX-License-Identifier: GPL-3.0-or-later
//! PNG 8/16 (`png` crate) with Blender's zlib level mapping (WP 5.4).

use crate::codec::{DecodedImage, ImageError};

/// Blender's PNG `compression` percent → zlib level (`format_png.cc`:
/// `int(float(compress) / 11.1111f)` clamped to 0–9).
pub fn png_zlib_level(compression: u8) -> u8 {
    ((f32::from(compression) / 11.1111) as i32).clamp(0, 9) as u8
}

pub(crate) fn encode_png(
    image: &DecodedImage,
    bit_depth: u8,
    compression: u8,
) -> Result<Vec<u8>, ImageError> {
    let color = match image.channels {
        1 => png::ColorType::Grayscale,
        2 => png::ColorType::GrayscaleAlpha,
        3 => png::ColorType::Rgb,
        _ => png::ColorType::Rgba,
    };
    let data: Vec<u8> = match bit_depth {
        8 => image.to_u8(),
        16 => image
            .to_u16()
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect(),
        other => {
            return Err(ImageError::Unsupported(format!("PNG bit depth {other}")));
        }
    };
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, image.width, image.height);
        enc.set_color(color);
        enc.set_depth(if bit_depth == 16 {
            png::BitDepth::Sixteen
        } else {
            png::BitDepth::Eight
        });
        let level = png_zlib_level(compression);
        enc.set_deflate_compression(if level == 0 {
            png::DeflateCompression::NoCompression
        } else {
            png::DeflateCompression::Level(level)
        });
        let mut writer = enc
            .write_header()
            .map_err(|e| ImageError::Encode(format!("PNG: {e}")))?;
        writer
            .write_image_data(&data)
            .map_err(|e| ImageError::Encode(format!("PNG: {e}")))?;
        writer
            .finish()
            .map_err(|e| ImageError::Encode(format!("PNG: {e}")))?;
    }
    Ok(out)
}
