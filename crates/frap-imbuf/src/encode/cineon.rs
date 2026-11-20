// SPDX-License-Identifier: GPL-3.0-or-later
//! Cineon (`file_format='CINEON'`, `.cin`; M5+M6 stabilization): Blender 5.2.4's own writer.
//!
//! Ported from blender/source/blender/imbuf/intern/cineon/cineonlib.cc (`cineonCreate`,
//! `fillCineonMainHeader`) and logImageCore.cc (`logImageSetDataRGBA`,
//! `convertRGBAToLogElement`, `getLinToSrgbLut`, `getLinToLogLut`, `logImageSetData10`,
//! `float_uint`), GPL-2.0-or-later, Copyright 1999-2001 David Hodson and Blender authors:
//!
//! - big-endian, a 2048-byte header (file, image and blank origination / film headers), the
//!   image right after it;
//! - three 10-bit channels (RGB only: `color_mode='BW'` cannot be written), packed three per
//!   32-bit word from bit 22 down (`offset` 22, 12, 2), each row padded to a word;
//! - the samples are the render's scene-linear floats turned into sRGB by a 1024-entry table,
//!   then into printing density by a second one (`transfer_PrintingDensity`, reference white
//!   685, black 95, gamma 1.7), each looked up at `float_uint(v, 1023)`. `use_cineon_log` makes
//!   no difference.
//!
//! The header's file name, creation date and time differ from Blender's (frap writes no date);
//! the image data is byte for byte Blender's.

use crate::codec::{DecodedImage, ImageError, PixelData};

const HEADER_LEN: usize = 2048;
const MAX: u32 = 1023;

/// `float_uint`.
fn float_uint(value: f32, max: u32) -> u32 {
    if value < 0.0 {
        return 0;
    }
    if value > 1.0 - 0.5 / max as f32 {
        return max;
    }
    (max as f32 * value + 0.5) as u32
}

/// `getLinToSrgbLut` (1024 entries).
fn lin_to_srgb_lut() -> Vec<f32> {
    (0..=MAX)
        .map(|i| {
            let col = i as f32 / MAX as f32;
            if col < 0.003_130_8 {
                if col < 0.0 { 0.0 } else { col * 12.92 }
            } else {
                1.055 * col.powf(1.0 / 2.4) - 0.055
            }
        })
        .collect()
}

/// `getLinToLogLut` with Cineon's element (`refHighQuantity` 2.048, max 1023) and image
/// (reference white 685, black 95, gamma 1.7).
fn lin_to_log_lut() -> Vec<f32> {
    let (white, black, gamma) = (685.0f32, 95.0f32, 1.7f32);
    let negative_film_gamma = 0.6f32;
    let max = MAX as f32;
    let step = 2.048f32 / max;
    let gain = max / (1.0 - 10f32.powf((black - white) * step / negative_film_gamma * gamma / 1.7));
    let offset = gain - max;
    (0..=MAX)
        .map(|i| {
            (white
                + ((i as f32 + offset) / gain).powf(1.7 / gamma).log10()
                    / (step / negative_film_gamma))
                / max
        })
        .collect()
}

/// Writes a Cineon file of a 3- or 4-channel scene-linear float image (alpha is dropped).
/// `name` is the header's file name.
pub(crate) fn encode_cineon(image: &DecodedImage, name: &str) -> Result<Vec<u8>, ImageError> {
    let PixelData::F32(px) = &image.pixels else {
        return Err(ImageError::Unsupported("Cineon: float samples only".into()));
    };
    let nc = usize::from(image.channels);
    if nc < 3 {
        return Err(ImageError::Unsupported(
            "Cineon: only RGB images can be written".into(),
        ));
    }
    let (w, h) = (image.width as usize, image.height as usize);
    let srgb = lin_to_srgb_lut();
    let log = lin_to_log_lut();
    let row_len = (w * 3 - 1) / 3 * 4 + 4; // `getRowLength`, 10 bits, packing 1
    let mut data = Vec::with_capacity(row_len * h);
    for y in 0..h {
        let mut row = vec![0u32; row_len / 4];
        let (mut index, mut pixel, mut offset) = (0usize, 0u32, 22i32);
        for x in 0..w {
            let p = &px[(y * w + x) * nc..(y * w + x) * nc + 3];
            for &v in p {
                let s = srgb[float_uint(v, MAX) as usize];
                let l = log[float_uint(s, MAX) as usize];
                pixel |= float_uint(l, MAX) << offset;
                offset -= 10;
                if offset < 0 {
                    row[index] = pixel;
                    index += 1;
                    pixel = 0;
                    offset = 22;
                }
            }
        }
        if pixel != 0 {
            row[index] = pixel;
        }
        for word in row {
            data.extend_from_slice(&word.to_be_bytes());
        }
    }
    let mut hd = vec![0u8; HEADER_LEN];
    let put32 =
        |hd: &mut Vec<u8>, at: usize, v: u32| hd[at..at + 4].copy_from_slice(&v.to_be_bytes());
    let text = |hd: &mut Vec<u8>, at: usize, len: usize, s: &str| {
        let b = s.as_bytes();
        let n = b.len().min(len - 1);
        hd[at..at + n].copy_from_slice(&b[..n]);
    };
    // --- file header
    put32(&mut hd, 0, 0x802A_5FD7);
    put32(&mut hd, 4, HEADER_LEN as u32); // image offset
    put32(&mut hd, 8, 192 + 520 + 312); // generic header size
    put32(&mut hd, 20, (HEADER_LEN + data.len()) as u32);
    text(&mut hd, 24, 8, "v4.5");
    text(&mut hd, 32, 100, name);
    // --- image header (192)
    hd[192] = 0; // orientation
    hd[193] = 3; // elements per image
    for i in 0..3 {
        let at = 196 + 28 * i;
        hd[at] = 0;
        hd[at + 1] = i as u8;
        hd[at + 2] = 10;
        put32(&mut hd, at + 4, w as u32);
        put32(&mut hd, at + 8, h as u32);
        put32(&mut hd, at + 12, 0); // reference low data
        put32(&mut hd, at + 16, 0.0f32.to_bits());
        put32(&mut hd, at + 20, MAX);
        put32(&mut hd, at + 24, 2.048f32.to_bits());
    }
    text(&mut hd, 452, 200, "Blender");
    hd[681] = 5; // packing
    let mut out = hd;
    out.extend_from_slice(&data);
    Ok(out)
}

/// Whether `bytes` start like a Cineon file.
pub fn is_cineon(bytes: &[u8]) -> bool {
    bytes.get(0..4) == Some(&[0x80, 0x2A, 0x5F, 0xD7])
}

/// Reads the 10-bit printing-density codes of a Cineon file written by frap or Blender (three
/// channels, packing 5), as stored.
pub fn decode_cineon(b: &[u8]) -> Result<DecodedImage, ImageError> {
    if !is_cineon(b) || b.len() < HEADER_LEN {
        return Err(ImageError::Unsupported("not a Cineon file".into()));
    }
    let be = |o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let offset = be(4) as usize;
    let (w, h) = (be(200) as usize, be(204) as usize);
    if b[193] != 3 || b[198] != 10 || b[681] != 5 {
        return Err(ImageError::Unsupported(
            "Cineon: only 3 x 10-bit filled data".into(),
        ));
    }
    let row_len = (w * 3 - 1) / 3 * 4 + 4;
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for i in 0..w * 3 {
            let o = offset + y * row_len + 4 * (i / 3);
            if o + 4 > b.len() {
                return Err(ImageError::Decode("Cineon: truncated image data".into()));
            }
            out.push(((be(o) >> (22 - 10 * (i % 3) as u32)) & 0x3FF) as u16);
        }
    }
    DecodedImage::new(w as u32, h as u32, 3, PixelData::U16(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luts_follow_blender() {
        assert_eq!(float_uint(-1.0, 1023), 0);
        assert_eq!(float_uint(0.9996, 1023), 1023);
        assert_eq!(float_uint(0.5, 1023), 512);
        let log = lin_to_log_lut();
        // Reference white (linear 1) is code 685, reference black is about 95.
        assert_eq!(float_uint(log[1023], 1023), 685);
        assert!((float_uint(log[0], 1023) as i32 - 95).abs() <= 1);
    }

    #[test]
    fn round_trips_codes() {
        let img = DecodedImage::new(5, 2, 4, PixelData::F32(vec![0.25; 40])).unwrap();
        let bytes = encode_cineon(&img, "a.cin").unwrap();
        assert!(is_cineon(&bytes));
        let back = decode_cineon(&bytes).unwrap();
        assert_eq!((back.width, back.height, back.channels), (5, 2, 3));
    }
}
