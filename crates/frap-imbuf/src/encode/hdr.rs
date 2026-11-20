// SPDX-License-Identifier: GPL-3.0-or-later
//! Radiance HDR (`image` crate; WP 5.4).

use crate::codec::{DecodedImage, ImageError};

pub(crate) fn encode_hdr(image: &DecodedImage) -> Result<Vec<u8>, ImageError> {
    // Radiance stores RGB floats: grey expands, alpha is dropped.
    let rgb = image.with_channels(3)?.to_f32();
    let pixels: Vec<image::Rgb<f32>> = rgb
        .chunks_exact(3)
        .map(|c| image::Rgb([c[0], c[1], c[2]]))
        .collect();
    let mut out = Vec::new();
    image::codecs::hdr::HdrEncoder::new(&mut out)
        .encode(&pixels, image.width as usize, image.height as usize)
        .map_err(|e| ImageError::Encode(format!("HDR: {e}")))?;
    Ok(out)
}
