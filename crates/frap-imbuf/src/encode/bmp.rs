// SPDX-License-Identifier: GPL-3.0-or-later
//! BMP, byte for byte as Blender 5.2.4 writes it (`format_bmp.cc` → OpenImageIO 3.1), and the
//! 32-bit reading rule of OpenImageIO that Blender loads BMPs with.
//!
//! Ported from OpenImageIO 3.1.13 `src/bmp.imageio/bmpoutput.cpp` and `bmpinput.cpp`
//! (Apache-2.0, Copyright Contributors to the OpenImageIO project), with Blender's spec
//! (`prepare_save_bmp`: grey + alpha is promoted to RGBA; `ResolutionUnit` `"m"` and the
//! rounded pixels per metre when the render has a `ppm`):
//!
//! - 14-byte file header and a Windows V3 (40-byte) info header, `BI_RGB`, bottom-up rows
//!   padded to 4 bytes; `biSizeImage` holds the pixel count (OIIO's value, not the byte size);
//! - one channel: 8 bits per pixel with a 256-entry grey palette; three and four channels: 24 and
//!   32 bits per pixel, BGR(A) (the fourth byte is alpha).

use crate::codec::{DecodedImage, ImageError, PixelData};

const BMP_HEADER_SIZE: u32 = 14;
const WINDOWS_V3: u32 = 40;

pub(crate) fn encode_bmp(
    image: &DecodedImage,
    ppm: Option<[f64; 2]>,
) -> Result<Vec<u8>, ImageError> {
    let promoted;
    let image = if image.channels == 2 {
        promoted = image.with_channels(4)?;
        &promoted
    } else {
        image
    };
    let n = image.channels as usize;
    let (w, h) = (image.width as usize, image.height as usize);
    if w > 65535 || h > 65535 {
        return Err(ImageError::Encode(format!(
            "BMP: {w}x{h} is larger than 65535x65535"
        )));
    }
    let data = image.to_u8();
    let scanline = w * n;
    let padded = scanline.div_ceil(4) * 4;
    let palette_size = if n == 1 { 4 * 256 } else { 0 };
    let data_size = padded * h;
    let offset = BMP_HEADER_SIZE + WINDOWS_V3 + palette_size as u32;
    let file_size = data_size as u64 + u64::from(offset);
    if file_size >= 1 << 32 {
        return Err(ImageError::Encode(
            "BMP does not support files over 4GB in size".into(),
        ));
    }
    let mut out = Vec::with_capacity(file_size as usize);
    // File header.
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    // Info header (V3).
    let (hres, vres) = match ppm {
        Some([x, y]) if x > 0.0 && y > 0.0 => (x.round() as i32, y.round() as i32),
        _ => (0, 0),
    };
    let (bpp, colours) = if n == 1 {
        (8u16, 256u32)
    } else {
        ((n * 8) as u16, 0)
    };
    out.extend_from_slice(&WINDOWS_V3.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&bpp.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((w * h) as i32).to_le_bytes()); // OIIO: image_pixels()
    out.extend_from_slice(&hres.to_le_bytes());
    out.extend_from_slice(&vres.to_le_bytes());
    out.extend_from_slice(&colours.to_le_bytes());
    out.extend_from_slice(&colours.to_le_bytes()); // important colours
    for i in 0..colours {
        let v = i as u8;
        out.extend_from_slice(&[v, v, v, 255]);
    }
    // Rows, bottom first. OIIO swaps R and B over the whole padded buffer (`i < size - 2`, step
    // `nchannels`), which may also swap padding bytes: zeros, so it changes nothing.
    let mut buf = vec![0u8; padded];
    for y in (0..h).rev() {
        buf[..scanline].copy_from_slice(&data[y * scanline..(y + 1) * scanline]);
        buf[scanline..].fill(0);
        if n >= 3 {
            let mut i = 0;
            while i + 2 < buf.len() {
                buf.swap(i, i + 2);
                i += n;
            }
        }
        out.extend_from_slice(&buf);
    }
    Ok(out)
}

/// The header of an uncompressed 32-bit BMP (`BI_RGB`, any info header from V3 on): pixel data
/// offset, width and signed height (negative: top-down).
fn header_32(bytes: &[u8]) -> Option<(usize, i32, i32)> {
    let u32_at = |o: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?))
    };
    let u16_at = |o: usize| -> Option<u16> {
        Some(u16::from_le_bytes(bytes.get(o..o + 2)?.try_into().ok()?))
    };
    if !bytes.starts_with(b"BM") {
        return None;
    }
    let offset = u32_at(10)? as usize;
    let header_size = u32_at(14)?;
    if header_size < WINDOWS_V3 || u16_at(28)? != 32 || u32_at(30)? != 0 {
        return None;
    }
    Some((offset, u32_at(18)? as i32, u32_at(22)? as i32))
}

/// Size of an uncompressed 32-bit BMP that [`decode_bmp_v3_32`] decodes (four channels).
pub fn probe_bmp_v3_32(bytes: &[u8]) -> Option<(u32, u32)> {
    let (_, w, h) = header_32(bytes)?;
    Some((w.max(0) as u32, h.unsigned_abs()))
}

/// Decodes an uncompressed 32-bit BMP (`BI_RGB`, any info header from V3 on) as OpenImageIO
/// does: four channels, the fourth byte being alpha (Blender writes alpha there). `None` for any
/// other BMP (decoded by the `image` crate, which drops that byte).
pub fn decode_bmp_v3_32(bytes: &[u8]) -> Option<Result<DecodedImage, ImageError>> {
    let (offset, width, height) = header_32(bytes)?;
    if width <= 0 || height == 0 || width > 1 << 16 || height.unsigned_abs() > 1 << 16 {
        return Some(Err(ImageError::Decode(format!(
            "BMP: invalid size {width}x{height}"
        ))));
    }
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    let row = w * 4;
    let mut data = vec![0u8; row * h];
    for y in 0..h {
        // Bottom-up unless the height is negative.
        let file_row = if height >= 0 { h - 1 - y } else { y };
        let start = offset + file_row * row;
        let Some(src) = bytes.get(start..start + row) else {
            return Some(Err(ImageError::Decode("BMP: truncated pixel data".into())));
        };
        let dst = &mut data[y * row..(y + 1) * row];
        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
        }
    }
    Some(DecodedImage::new(
        w as u32,
        h as u32,
        4,
        PixelData::U8(data),
    ))
}
