// SPDX-License-Identifier: GPL-3.0-or-later
//! SGI / IRIS (`file_format='IRIS'`, `.rgb`; M5+M6 stabilization): Blender 5.2.4 writes 8-bit
//! run-length encoded files through OpenImageIO 3.1 (`imbuf/intern/format_iris.cc`:
//! `compression` `rle` "to match historical behavior").
//!
//! Ported from OpenImageIO 3.1.13.1 `src/sgi.imageio/sgioutput.cpp` (`create_and_write_header`,
//! `write_scanline_rle`, `write_buffered_pixels`; Apache-2.0, Copyright Contributors to the
//! OpenImageIO Project), so the bytes equal Blender's: a 512-byte big-endian header, the
//! offset and length tables of every (channel, row) run (rows from the bottom), then the runs
//! in row-major, channel-minor order. The reader ([`decode_sgi`], after `sgiinput.cpp`) also
//! takes verbatim and 16-bit files.

use crate::codec::{DecodedImage, ImageError, PixelData};

/// `SGI_HEADER_LEN`.
const HEADER_LEN: usize = 512;
/// `SGI_MAGIC`.
const MAGIC: u16 = 474;

fn header(width: u32, height: u32, channels: u8, rle: bool) -> Vec<u8> {
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.extend_from_slice(&MAGIC.to_be_bytes());
    h.push(u8::from(rle)); // storage
    h.push(1); // bytes per channel
    let dimension: u16 = if height == 1 && channels == 1 {
        1
    } else if channels == 1 {
        2
    } else {
        3
    };
    h.extend_from_slice(&dimension.to_be_bytes());
    h.extend_from_slice(&(width as u16).to_be_bytes());
    h.extend_from_slice(&(height as u16).to_be_bytes());
    h.extend_from_slice(&u16::from(channels).to_be_bytes());
    h.extend_from_slice(&0i32.to_be_bytes()); // pixmin
    h.extend_from_slice(&255i32.to_be_bytes()); // pixmax
    h.extend_from_slice(&0i32.to_be_bytes()); // dummy
    h.extend_from_slice(&[0u8; 80]); // imagename (no `ImageDescription`)
    h.extend_from_slice(&0i32.to_be_bytes()); // colormap NORMAL
    h.resize(HEADER_LEN, 0);
    h
}

/// `SgiOutput::write_scanline_rle` for one channel of one 8-bit row (`row` holds every
/// channel; `c` is the channel, `xstride` the channel count).
fn rle_channel(row: &[u8], c: usize, xstride: usize, out: &mut Vec<u8>) {
    let cdata = &row[c..];
    let scanline_bytes = row.len();
    let at = |i: usize| cdata.get(i).copied().unwrap_or(0);
    let mut pos = 0usize;
    while pos < scanline_bytes {
        let mut start = pos;
        // Find the first run meeting a minimum length of 3.
        let (mut ahead_1, mut ahead_2) = (pos + xstride, pos + xstride * 2);
        while ahead_2 < scanline_bytes && (at(ahead_1) != at(ahead_2) || at(pos) != at(ahead_1)) {
            pos += xstride;
            ahead_1 += xstride;
            ahead_2 += xstride;
        }
        if ahead_2 >= scanline_bytes {
            // No more runs: the rest as literals.
            pos = scanline_bytes;
        }
        let mut count = (pos - start) / xstride;
        while count > 0 {
            let todo = count.min(127);
            count -= todo;
            out.push(0x80 | todo as u8);
            for _ in 0..todo {
                out.push(at(start));
                start += xstride;
            }
        }
        start = pos;
        if start >= scanline_bytes {
            break;
        }
        pos += xstride;
        while pos < scanline_bytes && at(start) == at(pos) {
            pos += xstride;
        }
        let mut count = (pos - start) / xstride;
        while count > 0 {
            let run = count.min(127);
            count -= run;
            out.push(run as u8);
            out.push(at(start));
        }
    }
    out.push(0);
}

/// Writes an 8-bit RLE SGI image of 1, 3 or 4 channels.
pub(crate) fn encode_sgi(image: &DecodedImage) -> Result<Vec<u8>, ImageError> {
    let PixelData::U8(px) = &image.pixels else {
        return Err(ImageError::Unsupported("IRIS: 8-bit samples only".into()));
    };
    let (w, h, nc) = (
        image.width as usize,
        image.height as usize,
        usize::from(image.channels),
    );
    if w > 65535 || h > 65535 {
        return Err(ImageError::Unsupported(format!("IRIS {w}x{h}")));
    }
    let table = h * nc;
    let mut offsets = vec![0u32; table];
    let mut lengths = vec![0u32; table];
    let mut data = Vec::new();
    let mut offset = (HEADER_LEN + 8 * table) as u32;
    let scanline = w * nc;
    // OIIO's row y counts from the bottom: file row y is image row h - 1 - y.
    for y in 0..h {
        let row = &px[(h - 1 - y) * scanline..(h - y) * scanline];
        for c in 0..nc {
            let before = data.len();
            rle_channel(row, c, nc, &mut data);
            let len = (data.len() - before) as u32;
            offsets[c * h + y] = offset;
            lengths[c * h + y] = len;
            offset += len;
        }
    }
    let mut out = header(image.width, image.height, image.channels, true);
    for v in offsets.iter().chain(&lengths) {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(&data);
    Ok(out)
}

/// Whether `bytes` start like an SGI image.
pub fn is_sgi(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER_LEN && bytes[0..2] == MAGIC.to_be_bytes() && bytes[2] <= 1
}

/// Reads an SGI image (verbatim or RLE, 8 or 16 bits per channel).
pub fn decode_sgi(b: &[u8]) -> Result<DecodedImage, ImageError> {
    if !is_sgi(b) {
        return Err(ImageError::Unsupported("not an SGI image".into()));
    }
    let bad = || ImageError::Decode("IRIS: truncated data".into());
    let be16 = |o: usize| -> Result<usize, ImageError> {
        let s = b.get(o..o + 2).ok_or_else(bad)?;
        Ok(usize::from(u16::from_be_bytes([s[0], s[1]])))
    };
    let be32 = |o: usize| -> Result<usize, ImageError> {
        let s = b.get(o..o + 4).ok_or_else(bad)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize)
    };
    let rle = b[2] == 1;
    let bpc = usize::from(b[3]);
    let dimension = be16(4)?;
    let w = be16(6)?;
    let h = if dimension == 1 { 1 } else { be16(8)? };
    let nc = if dimension < 3 { 1 } else { be16(10)? };
    if w == 0 || h == 0 || !(1..=4).contains(&nc) || !matches!(bpc, 1 | 2) {
        return Err(ImageError::Unsupported(format!(
            "IRIS {w}x{h}, {nc} channels of {bpc} bytes"
        )));
    }
    let mut samples = vec![0u16; w * h * nc];
    for c in 0..nc {
        for y in 0..h {
            let mut row = Vec::with_capacity(w);
            if rle {
                let k = c * h + y;
                let start = be32(HEADER_LEN + 4 * k)?;
                let mut i = start;
                let next = |i: &mut usize| -> Result<u16, ImageError> {
                    let v = if bpc == 1 {
                        u16::from(*b.get(*i).ok_or_else(bad)?)
                    } else {
                        be16(*i)? as u16
                    };
                    *i += bpc;
                    Ok(v)
                };
                while row.len() < w {
                    let code = next(&mut i)?;
                    let count = usize::from(code & 0x7F);
                    if count == 0 {
                        break;
                    }
                    if code & 0x80 != 0 {
                        for _ in 0..count {
                            row.push(next(&mut i)?);
                        }
                    } else {
                        let v = next(&mut i)?;
                        row.extend(std::iter::repeat_n(v, count));
                    }
                }
            } else {
                let start = HEADER_LEN + (c * h + y) * w * bpc;
                for x in 0..w {
                    row.push(if bpc == 1 {
                        u16::from(*b.get(start + x).ok_or_else(bad)?)
                    } else {
                        be16(start + 2 * x)? as u16
                    });
                }
            }
            row.resize(w, 0);
            let iy = h - 1 - y;
            for (x, v) in row.into_iter().enumerate() {
                samples[(iy * w + x) * nc + c] = v;
            }
        }
    }
    let pixels = if bpc == 1 {
        PixelData::U8(samples.into_iter().map(|v| v as u8).collect())
    } else {
        PixelData::U16(samples)
    };
    DecodedImage::new(w as u32, h as u32, nc as u8, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for nc in [1u8, 3, 4] {
            let (w, h) = (300u32, 5u32);
            let px: Vec<u8> = (0..(w * h) as usize * usize::from(nc))
                .map(|i| {
                    if (i / 17) % 3 == 0 {
                        9
                    } else {
                        (i % 251) as u8
                    }
                })
                .collect();
            let img = DecodedImage::new(w, h, nc, PixelData::U8(px)).unwrap();
            let bytes = encode_sgi(&img).unwrap();
            assert!(is_sgi(&bytes));
            assert_eq!(decode_sgi(&bytes).unwrap(), img);
        }
    }
}
