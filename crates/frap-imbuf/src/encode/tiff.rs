// SPDX-License-Identifier: GPL-3.0-or-later
//! TIFF (M5+M6 stabilization): the baseline layout Blender 5.2.4 writes through OpenImageIO 3.1
//! (`imbuf/intern/format_tiff.cc` → OIIO's `tiffoutput.cpp` → libtiff), and a reader for it.
//!
//! What Blender's files hold (checked on its files of one film, `tests/golden/fixtures/render/
//! output/film/tiff*.tif`):
//!
//! - one little-endian IFD; strips of 32 rows (`RowsPerStrip` 32), contiguous planar
//!   configuration, `Orientation` 1, unsigned integer samples (`SampleFormat` 1);
//! - `Compression` from `image_settings.tiff_codec`: `NONE` 1, `LZW` 5, `DEFLATE` 8 (Adobe
//!   deflate, zlib stream), `PACKBITS` 32773; LZW and deflate use horizontal differencing
//!   (`Predictor` 2);
//! - `Photometric` 1 (min-is-black) for one channel, 2 (RGB) otherwise; a fourth channel is
//!   alpha, `ExtraSamples` 2 (unassociated) for 8-bit files and 1 (associated) for 16-bit ones
//!   (`prepare_save_tiff`: `oiio:UnassociatedAlpha` 0 for 16-bit RGBA);
//! - `XResolution` / `YResolution` in pixels per inch (`ResolutionUnit` 2) from the render's
//!   pixels per metre.
//!
//! The compressed strips come from frap's encoders (zlib level 6 as libtiff's default, the
//! TIFF LZW code with early change, PackBits), not libtiff's: files equal Blender's by samples.
//! Blender's `DateTime` tag is not written.

use super::WriteOptions;
use crate::codec::{DecodedImage, ImageError, PixelData};

/// `image_settings.tiff_codec`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TiffCodec {
    /// `'NONE'`
    None,
    /// `'DEFLATE'` (Blender's default).
    #[default]
    Deflate,
    /// `'LZW'`
    Lzw,
    /// `'PACKBITS'`
    PackBits,
}

impl TiffCodec {
    /// Parses `tiff_codec`.
    pub fn from_identifier(id: &str) -> Option<TiffCodec> {
        Some(match id {
            "NONE" => TiffCodec::None,
            "DEFLATE" => TiffCodec::Deflate,
            "LZW" => TiffCodec::Lzw,
            "PACKBITS" => TiffCodec::PackBits,
            _ => return None,
        })
    }

    fn tag(self) -> u16 {
        match self {
            TiffCodec::None => 1,
            TiffCodec::Lzw => 5,
            TiffCodec::Deflate => 8,
            TiffCodec::PackBits => 32773,
        }
    }

    fn predictor(self) -> bool {
        matches!(self, TiffCodec::Lzw | TiffCodec::Deflate)
    }
}

/// OIIO's scanline TIFF strip height.
const ROWS_PER_STRIP: u32 = 32;

const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;

/// One IFD entry: tag, type, values (SHORT / LONG values, or numerator-denominator pairs).
struct Entry {
    tag: u16,
    ty: u16,
    values: Vec<u32>,
}

impl Entry {
    fn short(tag: u16, values: &[u32]) -> Entry {
        Entry {
            tag,
            ty: SHORT,
            values: values.to_vec(),
        }
    }
    fn long(tag: u16, values: Vec<u32>) -> Entry {
        Entry {
            tag,
            ty: LONG,
            values,
        }
    }
    fn count(&self) -> u32 {
        let n = self.values.len() as u32;
        if self.ty == RATIONAL { n / 2 } else { n }
    }
    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for v in &self.values {
            if self.ty == SHORT {
                out.extend_from_slice(&(*v as u16).to_le_bytes());
            } else {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }
}

/// A float as a TIFF rational (whole numbers exactly; otherwise over 10000).
fn rational(v: f64) -> [u32; 2] {
    if v.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&v) {
        [v as u32, 1]
    } else {
        [
            (v * 10000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32,
            10000,
        ]
    }
}

/// Horizontal differencing (`Predictor` 2) of one row of `spp` samples per pixel.
fn predict_row_u8(row: &mut [u8], spp: usize) {
    for i in (spp..row.len()).rev() {
        row[i] = row[i].wrapping_sub(row[i - spp]);
    }
}

fn predict_row_u16(row: &mut [u16], spp: usize) {
    for i in (spp..row.len()).rev() {
        row[i] = row[i].wrapping_sub(row[i - spp]);
    }
}

fn undo_predict_u8(row: &mut [u8], spp: usize) {
    for i in spp..row.len() {
        row[i] = row[i].wrapping_add(row[i - spp]);
    }
}

fn undo_predict_u16(row: &mut [u16], spp: usize) {
    for i in spp..row.len() {
        row[i] = row[i].wrapping_add(row[i - spp]);
    }
}

/// PackBits (Apple / TIFF 32773): runs of 2 to 128 equal bytes as `257 - n`, literals of up to
/// 128 bytes as `n - 1`.
fn packbits(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 128 + 1);
    let mut i = 0;
    while i < data.len() {
        let mut run = 1;
        while i + run < data.len() && run < 128 && data[i + run] == data[i] {
            run += 1;
        }
        if run >= 2 {
            out.push((257 - run) as u8);
            out.push(data[i]);
            i += run;
            continue;
        }
        let start = i;
        while i < data.len() && i - start < 128 {
            if i + 1 < data.len() && data[i + 1] == data[i] {
                break;
            }
            i += 1;
        }
        if i == start {
            i += 1;
        }
        out.push((i - start - 1) as u8);
        out.extend_from_slice(&data[start..i]);
    }
    out
}

fn unpackbits(data: &[u8], expected: usize) -> Result<Vec<u8>, ImageError> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i < data.len() && out.len() < expected {
        let n = data[i] as i8;
        i += 1;
        if n >= 0 {
            let len = n as usize + 1;
            let lit = data
                .get(i..i + len)
                .ok_or_else(|| corrupt("PackBits literal"))?;
            out.extend_from_slice(lit);
            i += len;
        } else if n != -128 {
            let b = *data.get(i).ok_or_else(|| corrupt("PackBits run"))?;
            out.extend(std::iter::repeat_n(b, (1 - i32::from(n)) as usize));
            i += 1;
        }
    }
    Ok(out)
}

/// TIFF LZW: MSB-first codes of 9 to 12 bits, `Clear` 256, `EndOfInformation` 257, the code
/// width growing one code early (libtiff's `tif_lzw.c` convention).
fn lzw_encode(data: &[u8]) -> Vec<u8> {
    struct Bits {
        out: Vec<u8>,
        acc: u32,
        n: u32,
    }
    impl Bits {
        fn put(&mut self, code: u16, width: u32) {
            self.acc = (self.acc << width) | u32::from(code);
            self.n += width;
            while self.n >= 8 {
                self.n -= 8;
                self.out.push((self.acc >> self.n) as u8);
            }
            self.acc &= (1 << self.n) - 1;
        }
        fn finish(mut self) -> Vec<u8> {
            if self.n > 0 {
                self.out.push((self.acc << (8 - self.n)) as u8);
            }
            self.out
        }
    }
    const CLEAR: u16 = 256;
    const EOI: u16 = 257;
    let mut bits = Bits {
        out: Vec::with_capacity(data.len() / 2 + 16),
        acc: 0,
        n: 0,
    };
    let mut dict: std::collections::HashMap<(u16, u8), u16> = std::collections::HashMap::new();
    let mut next: u16 = 258;
    let mut width = 9;
    bits.put(CLEAR, width);
    let Some((&first, rest)) = data.split_first() else {
        bits.put(EOI, width);
        return bits.finish();
    };
    let mut cur = u16::from(first);
    for &b in rest {
        if let Some(&code) = dict.get(&(cur, b)) {
            cur = code;
            continue;
        }
        bits.put(cur, width);
        dict.insert((cur, b), next);
        next += 1;
        if next == 4094 {
            bits.put(CLEAR, width);
            dict.clear();
            next = 258;
            width = 9;
        } else if u32::from(next) + 1 > (1 << width) {
            width += 1;
        }
        cur = u16::from(b);
    }
    bits.put(cur, width);
    // The decoder adds a table entry for this code too, and may widen before EOI.
    if u32::from(next) + 2 > (1 << width) && width < 12 {
        width += 1;
    }
    bits.put(EOI, width);
    bits.finish()
}

fn lzw_decode(data: &[u8], expected: usize) -> Result<Vec<u8>, ImageError> {
    let mut out = Vec::with_capacity(expected);
    let mut table: Vec<Vec<u8>> = Vec::with_capacity(4096);
    let reset = |table: &mut Vec<Vec<u8>>| {
        table.clear();
        table.extend((0..=255u8).map(|b| vec![b]));
        table.push(Vec::new());
        table.push(Vec::new());
    };
    reset(&mut table);
    let (mut acc, mut n, mut pos) = (0u32, 0u32, 0usize);
    let mut width = 9;
    let mut prev: Option<usize> = None;
    loop {
        while n < width {
            let Some(&b) = data.get(pos) else {
                return Ok(out);
            };
            acc = (acc << 8) | u32::from(b);
            n += 8;
            pos += 1;
        }
        let code = ((acc >> (n - width)) & ((1 << width) - 1)) as usize;
        n -= width;
        acc &= (1 << n) - 1;
        match code {
            256 => {
                reset(&mut table);
                width = 9;
                prev = None;
                continue;
            }
            257 => return Ok(out),
            _ => {}
        }
        let entry = if code < table.len() {
            table[code].clone()
        } else if let Some(p) = prev {
            let mut e = table[p].clone();
            e.push(table[p][0]);
            e
        } else {
            return Err(corrupt("LZW code"));
        };
        if let Some(p) = prev {
            let mut e = table[p].clone();
            e.push(entry[0]);
            table.push(e);
        }
        out.extend_from_slice(&entry);
        prev = Some(code);
        if table.len() + 1 >= (1 << width) && width < 12 {
            width += 1;
        }
        if out.len() >= expected {
            return Ok(out);
        }
    }
}

fn corrupt(what: &str) -> ImageError {
    ImageError::Decode(format!("TIFF: corrupt {what}"))
}

fn deflate(data: &[u8]) -> Result<Vec<u8>, ImageError> {
    use std::io::Write;
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
    z.write_all(data)
        .and_then(|_| z.finish())
        .map_err(|e| ImageError::Encode(format!("TIFF deflate: {e}")))
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, ImageError> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|e| ImageError::Decode(format!("TIFF deflate: {e}")))?;
    Ok(out)
}

/// Writes a TIFF of `image` (8- or 16-bit samples, 1, 3 or 4 channels).
pub(crate) fn encode_tiff(
    image: &DecodedImage,
    codec: TiffCodec,
    options: &WriteOptions,
) -> Result<Vec<u8>, ImageError> {
    let (w, h) = (image.width, image.height);
    let spp = usize::from(image.channels);
    if !matches!(spp, 1 | 3 | 4) {
        return Err(ImageError::Unsupported(format!("TIFF with {spp} channels")));
    }
    let bits: u32 = match &image.pixels {
        PixelData::U8(_) => 8,
        PixelData::U16(_) => 16,
        PixelData::F32(_) => {
            return Err(ImageError::Unsupported("TIFF with float samples".into()));
        }
    };
    let row_samples = w as usize * spp;
    let mut strips: Vec<Vec<u8>> = Vec::new();
    for s in 0..h.div_ceil(ROWS_PER_STRIP) {
        let (y0, y1) = (s * ROWS_PER_STRIP, ((s + 1) * ROWS_PER_STRIP).min(h));
        let range = y0 as usize * row_samples..y1 as usize * row_samples;
        let mut raw: Vec<u8> = Vec::with_capacity(range.len() * (bits as usize / 8));
        match &image.pixels {
            PixelData::U8(v) => {
                let mut part = v[range].to_vec();
                if codec.predictor() {
                    for row in part.chunks_exact_mut(row_samples) {
                        predict_row_u8(row, spp);
                    }
                }
                raw = part;
            }
            PixelData::U16(v) => {
                let mut part = v[range].to_vec();
                if codec.predictor() {
                    for row in part.chunks_exact_mut(row_samples) {
                        predict_row_u16(row, spp);
                    }
                }
                for x in part {
                    raw.extend_from_slice(&x.to_le_bytes());
                }
            }
            PixelData::F32(_) => unreachable!("rejected above"),
        }
        strips.push(match codec {
            TiffCodec::None => raw,
            TiffCodec::Deflate => deflate(&raw)?,
            TiffCodec::Lzw => lzw_encode(&raw),
            TiffCodec::PackBits => packbits(&raw),
        });
    }
    let photometric = if spp == 1 { 1 } else { 2 };
    let ppi = options
        .ppm
        .filter(|p| p[0] > 0.0 && p[1] > 0.0)
        .map(|p| [p[0] * 0.0254, p[1] * 0.0254])
        .unwrap_or([72.0, 72.0]);
    let mut entries = vec![
        Entry::short(256, &[w]),
        Entry::short(257, &[h]),
        Entry::short(258, &vec![bits; spp]),
        Entry::short(259, &[u32::from(codec.tag())]),
        Entry::short(262, &[photometric]),
        Entry::long(273, vec![0; strips.len()]),
        Entry::short(274, &[1]),
        Entry::short(277, &[spp as u32]),
        Entry::short(278, &[ROWS_PER_STRIP]),
        Entry::long(279, strips.iter().map(|s| s.len() as u32).collect()),
        Entry {
            tag: 282,
            ty: RATIONAL,
            values: rational(ppi[0]).to_vec(),
        },
        Entry {
            tag: 283,
            ty: RATIONAL,
            values: rational(ppi[1]).to_vec(),
        },
        Entry::short(284, &[1]),
        Entry::short(296, &[2]),
    ];
    if w > 0xFFFF || h > 0xFFFF {
        entries[0] = Entry::long(256, vec![w]);
        entries[1] = Entry::long(257, vec![h]);
    }
    if codec.predictor() {
        entries.push(Entry::short(317, &[2]));
    }
    if spp == 4 {
        entries.push(Entry::short(
            338,
            &[if options.tiff_associated_alpha { 1 } else { 2 }],
        ));
    }
    entries.push(Entry::short(339, &vec![1; spp]));
    // Layout: header, IFD, out-of-line values, strips.
    let ifd_len = 2 + 12 * entries.len() + 4;
    let mut extra_at = 8 + ifd_len;
    let mut extra_offsets = Vec::with_capacity(entries.len());
    for e in &entries {
        let len = e.bytes().len();
        if len > 4 {
            extra_offsets.push(Some(extra_at as u32));
            extra_at += len + (len & 1);
        } else {
            extra_offsets.push(None);
        }
    }
    let mut at = extra_at as u32;
    let offsets: Vec<u32> = strips
        .iter()
        .map(|s| {
            let o = at;
            at += s.len() as u32;
            o
        })
        .collect();
    entries[5].values = offsets;
    let mut out = Vec::with_capacity(at as usize);
    out.extend_from_slice(b"II*\0");
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut extra = Vec::new();
    for (e, off) in entries.iter().zip(&extra_offsets) {
        out.extend_from_slice(&e.tag.to_le_bytes());
        out.extend_from_slice(&e.ty.to_le_bytes());
        out.extend_from_slice(&e.count().to_le_bytes());
        let b = e.bytes();
        match off {
            Some(o) => {
                out.extend_from_slice(&o.to_le_bytes());
                extra.extend_from_slice(&b);
                if b.len() & 1 == 1 {
                    extra.push(0);
                }
            }
            None => {
                let mut v = [0u8; 4];
                v[..b.len()].copy_from_slice(&b);
                out.extend_from_slice(&v);
            }
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&extra);
    for s in &strips {
        out.extend_from_slice(s);
    }
    Ok(out)
}

/// The tags of a TIFF's first IFD that [`decode_tiff`] reads, by tag number.
fn read_ifd(b: &[u8]) -> Result<(bool, std::collections::BTreeMap<u16, Vec<u32>>), ImageError> {
    let le = match b.get(0..4) {
        Some(b"II*\0") => true,
        Some(b"MM\0*") => false,
        _ => return Err(ImageError::Unsupported("not a TIFF file".into())),
    };
    let u16_at = |o: usize| -> Result<u32, ImageError> {
        let s = b.get(o..o + 2).ok_or_else(|| corrupt("IFD"))?;
        let v = [s[0], s[1]];
        Ok(u32::from(if le {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        }))
    };
    let u32_at = |o: usize| -> Result<u32, ImageError> {
        let s = b.get(o..o + 4).ok_or_else(|| corrupt("IFD"))?;
        let v = [s[0], s[1], s[2], s[3]];
        Ok(if le {
            u32::from_le_bytes(v)
        } else {
            u32::from_be_bytes(v)
        })
    };
    let ifd = u32_at(4)? as usize;
    let n = u16_at(ifd)? as usize;
    let mut tags = std::collections::BTreeMap::new();
    for i in 0..n {
        let e = ifd + 2 + 12 * i;
        let tag = u16_at(e)? as u16;
        let ty = u16_at(e + 2)?;
        let count = u32_at(e + 4)? as usize;
        let size = match ty {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => continue,
        };
        let at = if size * count <= 4 {
            e + 8
        } else {
            u32_at(e + 8)? as usize
        };
        let mut values = Vec::with_capacity(count);
        for k in 0..count.min(1 << 20) {
            values.push(match size {
                1 => u32::from(*b.get(at + k).ok_or_else(|| corrupt("IFD value"))?),
                2 => u16_at(at + 2 * k)?,
                4 => u32_at(at + 4 * k)?,
                _ => u32_at(at + 8 * k)?,
            });
        }
        tags.insert(tag, values);
    }
    Ok((le, tags))
}

/// Reads a strip-organised, contiguous TIFF of 8- or 16-bit unsigned samples (1 to 4 channels)
/// compressed with none, LZW, deflate or PackBits, with or without horizontal differencing.
pub fn decode_tiff(b: &[u8]) -> Result<DecodedImage, ImageError> {
    let (le, tags) = read_ifd(b)?;
    let one = |t: u16, d: u32| tags.get(&t).and_then(|v| v.first().copied()).unwrap_or(d);
    let (w, h) = (one(256, 0), one(257, 0));
    let spp = one(277, 1) as usize;
    let bits = one(258, 1);
    let compression = one(259, 1);
    let predictor = one(317, 1);
    let rps = one(278, h.max(1)).max(1);
    if w == 0 || h == 0 || !(1..=4).contains(&spp) || !matches!(bits, 8 | 16) {
        return Err(ImageError::Unsupported(format!(
            "TIFF {w}x{h}, {spp} samples of {bits} bits"
        )));
    }
    if one(284, 1) != 1 || one(339, 1) != 1 || tags.contains_key(&322) {
        return Err(ImageError::Unsupported(
            "TIFF: planar, tiled or non-integer data".into(),
        ));
    }
    let offsets = tags.get(&273).cloned().unwrap_or_default();
    let counts = tags.get(&279).cloned().unwrap_or_default();
    let bps = bits as usize / 8;
    let row_bytes = w as usize * spp * bps;
    let mut data = Vec::with_capacity(row_bytes * h as usize);
    for (k, (&o, &c)) in offsets.iter().zip(&counts).enumerate() {
        let rows = rps.min(h.saturating_sub(k as u32 * rps)) as usize;
        let expected = rows * row_bytes;
        let src = b
            .get(o as usize..o as usize + c as usize)
            .ok_or_else(|| corrupt("strip"))?;
        let mut raw = match compression {
            1 => src.to_vec(),
            5 => lzw_decode(src, expected)?,
            8 | 32946 => inflate(src)?,
            32773 => unpackbits(src, expected)?,
            other => {
                return Err(ImageError::Unsupported(format!("TIFF compression {other}")));
            }
        };
        raw.resize(expected, 0);
        data.extend_from_slice(&raw);
    }
    data.resize(row_bytes * h as usize, 0);
    let pixels = if bits == 8 {
        if predictor == 2 {
            for row in data.chunks_exact_mut(row_bytes) {
                undo_predict_u8(row, spp);
            }
        }
        PixelData::U8(data)
    } else {
        let mut v: Vec<u16> = data
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        if predictor == 2 {
            for row in v.chunks_exact_mut(w as usize * spp) {
                undo_predict_u16(row, spp);
            }
        }
        PixelData::U16(v)
    };
    DecodedImage::new(w, h, spp as u8, pixels)
}

/// `ExtraSamples` of a TIFF (`None` without the tag): 1 associated alpha, 2 unassociated.
pub fn tiff_extra_samples(b: &[u8]) -> Option<u32> {
    read_ifd(b).ok()?.1.get(&338)?.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(channels: u8, pixels: PixelData, w: u32, h: u32) -> DecodedImage {
        DecodedImage::new(w, h, channels, pixels).unwrap()
    }

    #[test]
    fn round_trips_every_codec() {
        let (w, h) = (37, 70);
        for spp in [1u8, 3, 4] {
            let n = (w * h) as usize * usize::from(spp);
            let u8s: Vec<u8> = (0..n).map(|i| ((i * 7) / 5 % 256) as u8).collect();
            let u16s: Vec<u16> = (0..n).map(|i| ((i * 977) % 65536) as u16).collect();
            for codec in [
                TiffCodec::None,
                TiffCodec::Deflate,
                TiffCodec::Lzw,
                TiffCodec::PackBits,
            ] {
                for img in [
                    image(spp, PixelData::U8(u8s.clone()), w, h),
                    image(spp, PixelData::U16(u16s.clone()), w, h),
                ] {
                    let bytes = encode_tiff(&img, codec, &WriteOptions::default()).unwrap();
                    assert_eq!(decode_tiff(&bytes).unwrap(), img, "{codec:?} {spp}");
                }
            }
        }
    }

    #[test]
    fn lzw_long_runs_and_resets() {
        let data: Vec<u8> = (0..200_000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        assert_eq!(lzw_decode(&lzw_encode(&data), data.len()).unwrap(), data);
        let flat = vec![7u8; 100_000];
        assert_eq!(lzw_decode(&lzw_encode(&flat), flat.len()).unwrap(), flat);
        assert_eq!(lzw_decode(&lzw_encode(&[]), 0).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn packbits_runs() {
        let data = [1, 1, 1, 2, 3, 4, 4, 5, 5, 5, 5, 5, 6];
        assert_eq!(unpackbits(&packbits(&data), data.len()).unwrap(), data);
        let long: Vec<u8> = (0..1000).map(|i| (i / 300) as u8).collect();
        assert_eq!(unpackbits(&packbits(&long), long.len()).unwrap(), long);
    }
}
