// SPDX-License-Identifier: GPL-3.0-or-later
//! Targa, byte for byte as Blender 5.2.4 writes it (`format_targa.cc` → OpenImageIO 3.1).
//!
//! Ported from OpenImageIO 3.1.13 `src/targa.imageio/targaoutput.cpp` (Apache-2.0, Copyright
//! Contributors to the OpenImageIO project), with Blender's spec (`prepare_save_tga`:
//! `oiio:UnassociatedAlpha` 1, so samples are written as given; `compression` `"rle"` for
//! `'TARGA'`, `"none"` for `'TARGA_RAW'`; no metadata OIIO's Targa writer reads):
//!
//! - header: image type 3 (grey, never run-length encoded: OIIO encodes 3 and 4 channels only),
//!   10 (RLE true colour) or 2 (raw true colour); attribute bits 8 (alpha bits) when the
//!   channel count is even, plus the top-down flag 0x20 with RLE;
//! - pixels: BGR(A); RLE rows top first (OIIO's packet heuristics, kept as they are), raw rows
//!   bottom first;
//! - TGA 2.0 extension area (495 bytes, empty strings, OIIO's version 30113 as the software
//!   version, alpha type 3 "useful" with alpha) and the `TRUEVISION-XFILE.` footer.

use crate::codec::{DecodedImage, ImageError};

/// `OIIO_VERSION` of the OpenImageIO Blender 5.2.4 ships (3.1.13), written as the TGA 2.0
/// software version.
const OIIO_VERSION: u16 = 30113;
/// Size of the TGA 2.0 extension area.
const EXT_AREA_SIZE: u16 = 495;

pub(crate) fn encode_tga(image: &DecodedImage, rle: bool) -> Result<Vec<u8>, ImageError> {
    let n = image.channels as usize;
    let (w, h) = (image.width as usize, image.height as usize);
    if w > 65535 || h > 65535 {
        return Err(ImageError::Encode(format!(
            "Targa: {w}x{h} is larger than 65535x65535"
        )));
    }
    let data = image.to_u8();
    // OIIO: run-length encoding only for 3 and 4 channels.
    let want_rle = rle && n >= 3;
    let mut out = Vec::with_capacity(18 + w * h * n + 521);
    // Header (written member by member, little endian).
    out.push(0); // idlen
    out.push(0); // cmap_type
    out.push(if n <= 2 {
        3
    } else if want_rle {
        10
    } else {
        2
    });
    out.extend_from_slice(&0u16.to_le_bytes()); // cmap_first
    out.extend_from_slice(&0u16.to_le_bytes()); // cmap_length
    out.push(0); // cmap_size
    out.extend_from_slice(&0u16.to_le_bytes()); // x_origin
    out.extend_from_slice(&0u16.to_le_bytes()); // y_origin
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    out.push((n * 8) as u8);
    let mut attr = if n.is_multiple_of(2) { 8 } else { 0 };
    if want_rle {
        attr |= 0x20; // FLAG_Y_FLIP: top-down
    }
    out.push(attr);

    let row_bytes = w * n;
    if want_rle {
        for y in 0..h {
            write_rle_scanline(&mut out, &data[y * row_bytes..(y + 1) * row_bytes], w, n);
        }
    } else {
        // Raw rows are seeked to `18 + (height - y - 1) * w * n`: bottom row first.
        for y in (0..h).rev() {
            let row = &data[y * row_bytes..(y + 1) * row_bytes];
            if n <= 2 {
                out.extend_from_slice(row);
            } else {
                for px in row.chunks_exact(n) {
                    out.extend_from_slice(&[px[2], px[1], px[0]]);
                    if n > 3 {
                        out.push(px[3]);
                    }
                }
            }
        }
    }
    write_tga20_data_fields(&mut out, n);
    Ok(out)
}

/// `flush_rlp`: a run-length packet of `size` copies of `buf` (already BGR(A)).
fn flush_rlp(out: &mut Vec<u8>, buf: &[u8; 4], size: i32, n: usize) {
    if size < 1 {
        return;
    }
    out.push(((size - 1) | 0x80) as u8);
    out.extend_from_slice(&buf[..n]);
}

/// `flush_rawp`: a raw packet of `size` pixels of the scanline starting at pixel `start`.
fn flush_rawp(out: &mut Vec<u8>, src: &[u8], size: i32, start: i32, n: usize) {
    if size < 1 {
        return;
    }
    out.push(((size - 1) & !0x80) as u8);
    for i in 0..size {
        let p = ((start + i) as usize) * n;
        if n <= 2 {
            // OIIO writes `src + start` here for every pixel (it never runs: grey images are
            // not run-length encoded).
            out.extend_from_slice(&src[start as usize..start as usize + n]);
        } else {
            out.extend_from_slice(&[src[p + 2], src[p + 1], src[p]]);
            if n > 3 {
                out.push(src[p + 3]);
            }
        }
    }
}

/// `TGAOutput::write_scanline`'s run-length encoder, statement by statement.
fn write_rle_scanline(out: &mut Vec<u8>, bdata: &[u8], width: usize, n: usize) {
    let mut buf = [0u8; 4];
    let mut buf2 = [0u8; 4];
    let mut rlp = false;
    let mut rlcount: i32 = 0;
    let mut rawcount: i32 = 0;
    for x in 0..width {
        buf2 = buf;
        match n {
            3 => {
                buf[0] = bdata[x * 3 + 2];
                buf[1] = bdata[x * 3 + 1];
                buf[2] = bdata[x * 3];
            }
            _ => {
                buf[0] = bdata[x * 4 + 2];
                buf[1] = bdata[x * 4 + 1];
                buf[2] = bdata[x * 4];
                buf[3] = bdata[x * 4 + 3];
            }
        }
        if x == 0 {
            rlp = false;
            rlcount = 0;
            rawcount = 1;
            continue;
        }
        let x = x as i32;
        if rlp {
            if rlcount < 0x80 && buf == buf2 {
                rlcount += 1;
            } else {
                flush_rlp(out, &buf2, rlcount, n);
                rawcount += 1;
                rlcount -= 0x80;
                if rlcount < 0 {
                    rlcount = 0;
                }
                rlp = false;
            }
        } else {
            if rawcount > 0 && buf == buf2 {
                if rlcount == 0 {
                    rawcount -= 1;
                    rlcount += 1;
                }
                rlcount += 1;
            } else {
                rawcount += 1 + rlcount;
                rlcount = 0;
                if rawcount >= 0x80 {
                    rawcount -= 0x80;
                    flush_rawp(out, bdata, 0x80, x - 0x80 + 1, n);
                }
            }
            // `rlcount > 1 + 1 / nchannels` (integer division: 1 for one channel, else 0).
            if rlcount > 1 + 1 / n as i32 {
                flush_rawp(out, bdata, rawcount, x - rawcount - rlcount + 1, n);
                rawcount = 0;
                rlp = true;
            }
        }
    }
    if rlp {
        flush_rlp(out, &buf2, rlcount, n);
    } else {
        rawcount += rlcount;
        flush_rawp(out, bdata, rawcount, width as i32 - rawcount, n);
    }
}

/// `write_tga20_data_fields`: the extension area and the footer.
fn write_tga20_data_fields(out: &mut Vec<u8>, n: usize) {
    let ofs_ext = out.len() as u32;
    out.extend_from_slice(&EXT_AREA_SIZE.to_le_bytes());
    // Author (41), comment (324), timestamp (6 shorts), job ID (41), job time (3 shorts),
    // software ID (41): all empty.
    out.resize(out.len() + 41 + 324 + 12 + 41 + 6 + 41, 0);
    out.extend_from_slice(&OIIO_VERSION.to_le_bytes());
    out.push(0); // software version letter
    out.resize(out.len() + 4, 0); // key colour
    out.extend_from_slice(&[0; 4]); // pixel aspect ratio: 0/0 (square)
    out.extend_from_slice(&[0; 4]); // gamma: 0/0 (no `oiio:ColorSpace` in Blender's spec)
    out.extend_from_slice(&[0; 4]); // colour correction table offset
    out.extend_from_slice(&0u32.to_le_bytes()); // thumbnail offset
    out.extend_from_slice(&[0; 4]); // scanline table offset
    out.push(if n.is_multiple_of(2) { 3 } else { 0 }); // TGA_ALPHA_USEFUL / TGA_ALPHA_NONE
    out.extend_from_slice(&ofs_ext.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // developer area offset
    out.extend_from_slice(b"TRUEVISION-XFILE.\0");
}
