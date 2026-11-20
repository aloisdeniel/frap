// SPDX-License-Identifier: GPL-3.0-or-later
//! Encoders per output format (ARCHITECTURE §19.4: PNG 8/16, JPEG, WEBP, OPEN_EXR, HDR, TARGA,
//! BMP). Owned by WP 5.4 (`crates/frap-imbuf/src/encode/**`, reassigned in M5; the M5+M6 W0
//! integrator moved the M1 encoders here from `formats.rs` unchanged). [`encode`] is called by
//! `formats::encode` after the buffer check, so every encoder receives a consistent image.

mod exr;
mod hdr;
mod jpeg;
mod png;
mod raw8;
mod webp;

pub use png::png_zlib_level;

use crate::codec::{DecodedImage, EncodeFormat, ImageError};

/// Encodes a checked image (`formats::encode` validated channels and buffer size).
pub(crate) fn encode(image: &DecodedImage, format: &EncodeFormat) -> Result<Vec<u8>, ImageError> {
    match format {
        EncodeFormat::Png {
            bit_depth,
            compression,
        } => png::encode_png(image, *bit_depth, *compression),
        EncodeFormat::Jpeg { quality } => jpeg::encode_jpeg(image, *quality),
        EncodeFormat::WebP { lossless, quality } => webp::encode_webp(image, *lossless, *quality),
        EncodeFormat::OpenExr { bit_depth } => exr::encode_exr(image, *bit_depth),
        EncodeFormat::Hdr => hdr::encode_hdr(image),
        EncodeFormat::Targa => raw8::encode_8bit(image, ::image::ImageFormat::Tga),
        EncodeFormat::Bmp => raw8::encode_8bit(image, ::image::ImageFormat::Bmp),
    }
}
