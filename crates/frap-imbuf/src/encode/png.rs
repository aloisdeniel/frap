// SPDX-License-Identifier: GPL-3.0-or-later
//! PNG 8/16 (`png` crate) with Blender's zlib level mapping and the chunks OpenImageIO's PNG
//! writer adds for Blender's render output (WP 5.4).
//!
//! Blender 5.2.4 (`format_png.cc` → OIIO 3.1.13 `pngoutput.cpp` / `png_pvt.h::write_info`,
//! Apache-2.0): the zlib level is `int(compress / 11.1111f)` clamped to 0–9; display-referred
//! sRGB samples get `sRGB` (absolute colorimetric intent), `gAMA` and `cHRM`
//! (`png_set_sRGB_gAMA_and_cHRM`); a render's resolution (`ResolutionUnit` `"in"`,
//! `XResolution = float(ppm * 0.0254)`) becomes `pHYs` in pixels per metre, computed in float
//! and truncated (72 dpi → 2834). The deflate stream differs from libpng's, so files are equal
//! to Blender's by samples, not bytes; the stamp `tEXt` chunks, `oFFs` and `eXIf` are not
//! written.

use super::WriteOptions;
use crate::codec::{DecodedImage, ImageError};

/// Blender's PNG `compression` percent → zlib level (`format_png.cc`:
/// `int(float(compress) / 11.1111f)` clamped to 0–9).
pub fn png_zlib_level(compression: u8) -> u8 {
    ((f32::from(compression) / 11.1111) as i32).clamp(0, 9) as u8
}

/// `png_pvt::write_info`'s `pHYs` from Blender's inch resolution (`float` arithmetic).
fn phys_from_ppm(ppm: f64) -> u32 {
    let res_in_inches = (ppm * 0.0254) as f32;
    let scale = (100.0 / 2.54) as f32;
    (res_in_inches * scale) as u32
}

pub(crate) fn encode_png(
    image: &DecodedImage,
    bit_depth: u8,
    compression: u8,
    options: &WriteOptions,
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
        if options.srgb {
            enc.set_source_srgb(png::SrgbRenderingIntent::AbsoluteColorimetric);
            enc.set_source_gamma(png::ScaledFloat::from_scaled(45455));
            let s = png::ScaledFloat::from_scaled;
            enc.set_source_chromaticities(png::SourceChromaticities {
                white: (s(31270), s(32900)),
                red: (s(64000), s(33000)),
                green: (s(30000), s(60000)),
                blue: (s(15000), s(6000)),
            });
        }
        if let Some([x, y]) = options.ppm.filter(|p| p[0] > 0.0 && p[1] > 0.0) {
            enc.set_pixel_dims(Some(png::PixelDimensions {
                xppu: phys_from_ppm(x),
                yppu: phys_from_ppm(y),
                unit: png::Unit::Meter,
            }));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seventy_two_dpi_is_2834_pixels_per_metre() {
        assert_eq!(phys_from_ppm(72.0 / 0.0254), 2834);
    }
}
