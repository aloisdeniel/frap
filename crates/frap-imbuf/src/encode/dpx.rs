// SPDX-License-Identifier: GPL-3.0-or-later
//! DPX (`file_format='DPX'`; M5+M6 stabilization): Blender 5.2.4 writes it through OpenImageIO
//! 3.1 (`imbuf/intern/format_dpx.cc` → OIIO's `dpxoutput.cpp` → libdpx).
//!
//! Ported from OpenImageIO 3.1.13.1 `src/dpx.imageio/dpxoutput.cpp` (`open`, `prep_subimage`)
//! and `src/dpx.imageio/libdpx/{Writer.cpp,WriterInternal.h,DPXHeader.cpp}` (BSD-3-Clause,
//! Copyright Patrick A. Palmer and Contributors to the OpenImageIO Project):
//!
//! - native (little-endian) byte order, a 2048-byte header with libdpx's "undefined" fills,
//!   `0xFF` padding to the 8 KiB image offset, one element, rows top first;
//! - 8 and 16 bits: the samples as they are ("Packed"); 12 bits and one-channel 10 bits:
//!   `WritePackedMethod` (samples LSB first across 32-bit words, each row padded to a word);
//!   three- and four-channel 10 bits: "Filled, method A", three samples per word from the high
//!   bits down (libdpx's datum swap), two zero bits at the bottom;
//! - 10 and 12 bits are the top bits of the 16-bit samples OIIO converts to (`v >> 6`,
//!   `v >> 4`);
//! - Blender's element settings (`prepare_save_dpx`): white level `685 / 1023 · max`, black
//!   level `95 / 1023 · max`, transfer `Linear` (high quantity `max`) or, with
//!   `use_cineon_log`, `Printing density` (high quantity 2.048); low data 0; high data
//!   "undefined" (Blender passes it as a float, libdpx reads an int).
//!
//! The header's file name, creation time and creator differ from Blender's (frap writes the
//! file's base name, no time and `frap`); the image data is byte for byte Blender's.

use crate::codec::{DecodedImage, ImageError, PixelData};

/// The DPX settings of `image_settings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DpxSettings {
    /// `color_depth`: 8, 10, 12 or 16.
    pub bits: u8,
    /// `use_cineon_log`.
    pub log: bool,
}

const IMAGE_OFFSET: usize = 0x2000;
const UNDEF_F32: u32 = 0x7FC0_0000; // quiet NaN
const UNDEF_U32_AS_F32: f32 = 4_294_967_296.0;

struct Header(Vec<u8>);

impl Header {
    fn u8(&mut self, at: usize, v: u8) {
        self.0[at] = v;
    }
    fn u16(&mut self, at: usize, v: u16) {
        self.0[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, at: usize, v: u32) {
        self.0[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, at: usize, v: f32) {
        self.u32(at, v.to_bits());
    }
    fn nan(&mut self, at: usize) {
        self.u32(at, UNDEF_F32);
    }
    fn text(&mut self, at: usize, len: usize, s: &str) {
        let b = s.as_bytes();
        let n = b.len().min(len - 1);
        self.0[at..at + n].copy_from_slice(&b[..n]);
    }
}

/// libdpx's `Header` after OIIO's `open` with Blender's attributes (offsets of SMPTE 268M).
fn header(
    w: u32,
    h: u32,
    channels: u8,
    s: DpxSettings,
    packing: u16,
    file_size: u32,
    name: &str,
) -> Vec<u8> {
    let mut hd = Header(vec![0u8; 2048]);
    // --- generic file header
    hd.0[0..4].copy_from_slice(b"XPDS"); // 'SDPX' in little-endian order
    hd.u32(4, IMAGE_OFFSET as u32);
    hd.text(8, 8, "V2.0");
    hd.u32(16, file_size);
    hd.u32(20, 1); // ditto key
    hd.u32(24, 1664); // generic header size
    hd.u32(28, 384); // industry header size
    hd.u32(32, 0); // user data size
    hd.text(36, 100, name);
    hd.text(160, 100, "frap");
    hd.u32(660, 0xFFFF_FFFF); // encryption key
    // --- image information header
    hd.u16(768, 0); // left to right, top to bottom
    hd.u16(770, 1); // one element
    hd.u32(772, w);
    hd.u32(776, h);
    let max = (1u32 << s.bits) as f32 - 1.0;
    for e in 0..8 {
        let at = 780 + 72 * e;
        if e == 0 {
            hd.u32(at, 0); // unsigned
            hd.u32(at + 4, 0); // low data
            hd.f32(at + 8, 0.0); // low quantity
            hd.u32(at + 12, 0xFFFF_FFFF); // high data
            hd.f32(at + 16, if s.log { 2.048 } else { max });
            hd.u8(
                at + 20,
                match channels {
                    1 => 6,  // luma
                    3 => 50, // RGB
                    _ => 51, // RGBA
                },
            );
            hd.u8(at + 21, if s.log { 1 } else { 2 }); // printing density / linear
            hd.u8(at + 22, 0); // colorimetric: user defined
            hd.u8(at + 23, s.bits);
            hd.u16(at + 24, packing);
            hd.u16(at + 26, 0); // no encoding
            hd.u32(at + 28, IMAGE_OFFSET as u32);
            hd.u32(at + 32, 0);
            hd.u32(at + 36, 0);
        } else {
            for k in [0, 4, 12, 28, 32, 36] {
                hd.u32(at + k, 0xFFFF_FFFF);
            }
            hd.f32(at + 8, UNDEF_U32_AS_F32);
            hd.f32(at + 16, UNDEF_U32_AS_F32);
            hd.u32(at + 20, 0xFFFF_FFFF);
            hd.u32(at + 24, 0xFFFF_FFFF);
        }
    }
    // --- image source information header (1408)
    hd.u32(1408, 0); // x offset
    hd.u32(1412, 0); // y offset
    hd.nan(1416);
    hd.nan(1420);
    hd.u32(1424, w); // original size
    hd.u32(1428, h);
    for k in 0..4 {
        hd.u16(1620 + 2 * k, 0xFFFF); // border validity
    }
    hd.u32(1628, 1); // pixel aspect 1:1
    hd.u32(1632, 1);
    hd.nan(1636);
    hd.nan(1640);
    // --- motion-picture film header (1664)
    hd.u32(1712, 0xFFFF_FFFF); // frame position
    hd.u32(1716, 0xFFFF_FFFF); // sequence length
    hd.u32(1720, 0xFFFF_FFFF); // held count
    hd.nan(1724);
    hd.nan(1728);
    // --- television header (1920)
    hd.u32(1920, 0xFFFF_FFFF); // time code
    hd.u32(1924, 0xFFFF_FFFF); // user bits
    hd.u8(1928, 0xFF); // interlace
    hd.u8(1929, 0xFF); // field number
    hd.u8(1930, 0); // video signal
    hd.u8(1931, 0xFF);
    for at in [1932, 1936, 1940, 1944, 1948] {
        hd.nan(at);
    }
    hd.f32(1952, 95.0f32 / 1023.0 * max); // black level
    hd.nan(1956);
    hd.nan(1960);
    hd.f32(1964, 685.0f32 / 1023.0 * max); // white level
    hd.nan(1968);
    hd.0
}

/// A row of 16-bit samples as 32-bit words, the in-place view `WriterInternal.h` packs into.
struct Words<'a>(&'a mut [u16]);

impl Words<'_> {
    fn get(&self, d: usize) -> u32 {
        u32::from(self.0[2 * d]) | (u32::from(self.0[2 * d + 1]) << 16)
    }
    fn set(&mut self, d: usize, v: u32) {
        self.0[2 * d] = v as u16;
        self.0[2 * d + 1] = (v >> 16) as u16;
    }
}

/// `WritePackedMethod<U16, BITDEPTH>` on one row (in place, as libdpx does): the number of
/// 16-bit units written.
fn packed_method(buf: &mut [u16], len: usize, bits: u32) -> usize {
    let shift = 16 - bits;
    let mask: u32 = (1 << bits) - 1;
    for i in 0..len {
        let value = u32::from(buf[i]) >> shift;
        let mut w = Words(buf);
        let div = (i * bits as usize) / 32;
        let rem = ((i * bits as usize) % 32) as u32;
        let shift_mask = mask << rem;
        let cur = w.get(div);
        w.set(div, (cur & !shift_mask) | ((value << rem) & shift_mask));
        let carry = bits as i32 - (32 - rem as i32);
        if carry > 0 {
            let save = bits - carry as u32;
            let next = w.get(div + 1);
            w.set(
                div + 1,
                (next & !(mask >> save)) | ((value >> save) & (mask >> save)),
            );
        }
    }
    (len * bits as usize).div_ceil(32) * 2
}

/// `WritePackedMethodAB_10bit<U16, kFilledMethodA>` with `reverse` on one row.
fn filled_method_a(buf: &mut [u16], len: usize, reverse: bool) -> usize {
    let mut value: u32 = 0;
    for i in 0..len {
        let div = i / 3;
        let mut rem = (i % 3) as u32;
        if i > 0 && rem == 0 {
            Words(buf).set(div - 1, value);
            value = 0;
        }
        if reverse {
            rem = 2 - rem;
        }
        let comp = ((u32::from(buf[i]) >> 6) << (10 * rem)) << 2;
        let mask = (0x3FFu32 << (10 * rem)) << 2;
        value = (value & !mask) | (comp & mask);
    }
    Words(buf).set(len.div_ceil(3) - 1, value);
    len.div_ceil(3) * 2
}

/// Writes a DPX of `image` (8-bit samples for 8 bits, 16-bit samples otherwise). `name` is the
/// header's file name.
pub(crate) fn encode_dpx(
    image: &DecodedImage,
    s: DpxSettings,
    name: &str,
) -> Result<Vec<u8>, ImageError> {
    let (w, h, nc) = (image.width, image.height, usize::from(image.channels));
    if !matches!(nc, 1 | 3 | 4) {
        return Err(ImageError::Unsupported(format!("DPX with {nc} channels")));
    }
    let len = w as usize * nc;
    let mut data: Vec<u8> = Vec::new();
    let packing = match (s.bits, &image.pixels) {
        (8, PixelData::U8(px)) => {
            data.extend_from_slice(px);
            0
        }
        (16, PixelData::U16(px)) => {
            for v in px {
                data.extend_from_slice(&v.to_le_bytes());
            }
            0
        }
        (10 | 12, PixelData::U16(px)) => {
            let method_a = s.bits == 10 && nc != 1;
            for row in px.chunks_exact(len) {
                let mut buf = row.to_vec();
                buf.push(0);
                if buf.len() % 2 == 1 {
                    buf.push(0);
                }
                let units = if method_a {
                    filled_method_a(&mut buf, len, true)
                } else {
                    packed_method(&mut buf, len, u32::from(s.bits))
                };
                for v in &buf[..units] {
                    data.extend_from_slice(&v.to_le_bytes());
                }
            }
            u16::from(method_a)
        }
        (bits, _) => {
            return Err(ImageError::Unsupported(format!(
                "DPX: {bits}-bit output from these samples"
            )));
        }
    };
    let size = IMAGE_OFFSET + data.len();
    let mut out = header(w, h, image.channels, s, packing, size as u32, name);
    out.resize(IMAGE_OFFSET, 0xFF);
    out.extend_from_slice(&data);
    Ok(out)
}

/// Whether `bytes` start like a DPX file (either byte order).
pub fn is_dpx(bytes: &[u8]) -> bool {
    matches!(bytes.get(0..4), Some(b"SDPX" | b"XPDS"))
}

/// Reads the image element of a DPX file written by frap or by Blender (OIIO): the code values
/// as they are stored (8-bit samples, or 16-bit samples holding 10-, 12- or 16-bit codes).
pub fn decode_dpx(b: &[u8]) -> Result<DecodedImage, ImageError> {
    if !is_dpx(b) || b.len() < 2048 {
        return Err(ImageError::Unsupported("not a DPX file".into()));
    }
    let le = &b[0..4] == b"XPDS";
    let u16_at = |o: usize| {
        let v = [b[o], b[o + 1]];
        if le {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        }
    };
    let u32_at = |o: usize| {
        let v = [b[o], b[o + 1], b[o + 2], b[o + 3]];
        if le {
            u32::from_le_bytes(v)
        } else {
            u32::from_be_bytes(v)
        }
    };
    let (w, h) = (u32_at(772), u32_at(776));
    let nc = match b[800] {
        6 => 1,
        50 => 3,
        51 => 4,
        d => return Err(ImageError::Unsupported(format!("DPX descriptor {d}"))),
    };
    let bits = u32::from(b[803]);
    let packing = u16_at(804);
    let offset = u32_at(808) as usize;
    let len = w as usize * nc;
    let n = len * h as usize;
    let bad = || ImageError::Decode("DPX: truncated image data".into());
    let pixels = match bits {
        8 => PixelData::U8(b.get(offset..offset + n).ok_or_else(bad)?.to_vec()),
        16 => {
            let src = b.get(offset..offset + 2 * n).ok_or_else(bad)?;
            PixelData::U16(src.chunks_exact(2).map(|c| u16_at_slice(c, le)).collect())
        }
        10 | 12 => {
            let words_per_row = if bits == 10 && packing == 1 {
                len.div_ceil(3)
            } else {
                (len * bits as usize).div_ceil(32)
            };
            let mut out = Vec::with_capacity(n);
            for y in 0..h as usize {
                let row = offset + y * words_per_row * 4;
                let word = |d: usize| -> Result<u32, ImageError> {
                    let o = row + 4 * d;
                    b.get(o..o + 4).ok_or_else(bad)?;
                    Ok(u32_at(o))
                };
                for i in 0..len {
                    let v = if bits == 10 && packing == 1 {
                        (word(i / 3)? >> (2 + 10 * (2 - (i % 3) as u32))) & 0x3FF
                    } else {
                        let bit = i * bits as usize;
                        let (d, r) = (bit / 32, (bit % 32) as u32);
                        let lo = u64::from(word(d)?);
                        let hi = if r + bits > 32 {
                            u64::from(word(d + 1)?)
                        } else {
                            0
                        };
                        (((hi << 32 | lo) >> r) & ((1 << bits) - 1)) as u32
                    };
                    out.push(v as u16);
                }
            }
            PixelData::U16(out)
        }
        other => return Err(ImageError::Unsupported(format!("DPX {other}-bit data"))),
    };
    DecodedImage::new(w, h, nc as u8, pixels)
}

fn u16_at_slice(c: &[u8], le: bool) -> u16 {
    if le {
        u16::from_le_bytes([c[0], c[1]])
    } else {
        u16::from_be_bytes([c[0], c[1]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_depth() {
        let (w, h) = (13u32, 4u32);
        for nc in [1u8, 3, 4] {
            let n = (w * h) as usize * usize::from(nc);
            for bits in [8u8, 10, 12, 16] {
                let s = DpxSettings { bits, log: false };
                let img = if bits == 8 {
                    DecodedImage::new(w, h, nc, PixelData::U8((0..n).map(|i| i as u8).collect()))
                } else {
                    DecodedImage::new(
                        w,
                        h,
                        nc,
                        PixelData::U16((0..n).map(|i| (i * 4099 % 65536) as u16).collect()),
                    )
                }
                .unwrap();
                let bytes = encode_dpx(&img, s, "t.dpx").unwrap();
                let back = decode_dpx(&bytes).unwrap();
                let shift = 16 - u32::from(bits);
                let expect = match &img.pixels {
                    PixelData::U16(v) if bits < 16 => {
                        PixelData::U16(v.iter().map(|x| x >> shift).collect())
                    }
                    p => p.clone(),
                };
                assert_eq!(back.pixels, expect, "{nc} channels, {bits} bits");
            }
        }
    }
}
