// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-format codecs (PNG 8/16 via `png`, EXR via `exr`, the rest via `image`). WP 1.6.
//!
//! [`decode`] sniffs the format from the bytes and keeps the file's sample type (8-bit, 16-bit
//! or float) and channel count (1–4). [`encode`] converts the image to what the target format
//! stores, with Blender's rounding (`convert`). Images are top row first; samples are stored
//! as in the file (no colour-space or alpha conversion; EXR is conventionally premultiplied).
//! Encoders are deterministic: the same image and options give the same bytes.

use std::io::Cursor;
use std::path::Path;

use crate::codec::{DecodedImage, EncodeFormat, ImageCodec, ImageError, PixelData};
pub use crate::encode::png_zlib_level;

/// `image_settings.file_format` values frap reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileFormat {
    /// `'PNG'`
    Png,
    /// `'JPEG'`
    Jpeg,
    /// `'WEBP'`
    WebP,
    /// `'OPEN_EXR'`
    OpenExr,
    /// `'HDR'`
    Hdr,
    /// `'TARGA'`
    Targa,
    /// `'BMP'`
    Bmp,
    /// `'TIFF'`: written by render output only (not in [`FileFormat::ALL`], not sniffed by
    /// [`FileFormat::detect`]: frap does not load TIFF images yet; M5+M6 stabilization).
    Tiff,
    /// `'IRIS'` (render output only, as `Tiff`).
    Iris,
    /// `'DPX'` (render output only, as `Tiff`).
    Dpx,
    /// `'CINEON'` (render output only, as `Tiff`).
    Cineon,
    /// KTX2 (`KHR_texture_basisu`, M7 W0): read only, not a Blender `file_format` (Blender
    /// cannot load KTX2; not in [`FileFormat::ALL`]); decoded by [`crate::ktx2`] (WP 7.3).
    Ktx2,
}

impl FileFormat {
    /// Every format, in Blender's enum order of the ones frap supports.
    pub const ALL: [FileFormat; 7] = [
        FileFormat::Bmp,
        FileFormat::Png,
        FileFormat::Jpeg,
        FileFormat::Targa,
        FileFormat::OpenExr,
        FileFormat::Hdr,
        FileFormat::WebP,
    ];

    /// Blender's `file_format` identifier.
    pub fn identifier(self) -> &'static str {
        match self {
            FileFormat::Png => "PNG",
            FileFormat::Jpeg => "JPEG",
            FileFormat::WebP => "WEBP",
            FileFormat::OpenExr => "OPEN_EXR",
            FileFormat::Hdr => "HDR",
            FileFormat::Targa => "TARGA",
            FileFormat::Bmp => "BMP",
            FileFormat::Tiff => "TIFF",
            FileFormat::Iris => "IRIS",
            FileFormat::Dpx => "DPX",
            FileFormat::Cineon => "CINEON",
            FileFormat::Ktx2 => "KTX2",
        }
    }

    /// Parses Blender's `file_format` identifier.
    pub fn from_identifier(id: &str) -> Option<FileFormat> {
        FileFormat::ALL.into_iter().find(|f| f.identifier() == id)
    }

    /// The extension Blender appends with `use_file_extension` (`imb_file_extensions_*`, first
    /// entry).
    pub fn extension(self) -> &'static str {
        match self {
            FileFormat::Png => ".png",
            FileFormat::Jpeg => ".jpg",
            FileFormat::WebP => ".webp",
            FileFormat::OpenExr => ".exr",
            FileFormat::Hdr => ".hdr",
            FileFormat::Targa => ".tga",
            FileFormat::Bmp => ".bmp",
            FileFormat::Tiff => ".tif",
            FileFormat::Iris => ".rgb",
            FileFormat::Dpx => ".dpx",
            FileFormat::Cineon => ".cin",
            FileFormat::Ktx2 => ".ktx2",
        }
    }

    /// Sniffs the format from the first bytes. TGA has no magic number: it is assumed only when
    /// nothing else matches and the header is plausible.
    pub fn detect(bytes: &[u8]) -> Option<FileFormat> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(FileFormat::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(FileFormat::Jpeg)
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(FileFormat::WebP)
        } else if bytes.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
            Some(FileFormat::OpenExr)
        } else if bytes.starts_with(b"#?RADIANCE") || bytes.starts_with(b"#?RGBE") {
            Some(FileFormat::Hdr)
        } else if bytes.starts_with(b"BM") {
            Some(FileFormat::Bmp)
        } else if crate::ktx2::is_ktx2(bytes) {
            Some(FileFormat::Ktx2)
        } else if looks_like_tga(bytes) {
            Some(FileFormat::Targa)
        } else {
            None
        }
    }
}

fn looks_like_tga(b: &[u8]) -> bool {
    if b.len() < 18 {
        return false;
    }
    let color_map_type = b[1];
    let image_type = b[2];
    let depth = b[16];
    color_map_type <= 1
        && matches!(image_type, 1 | 2 | 3 | 9 | 10 | 11)
        && matches!(depth, 8 | 15 | 16 | 24 | 32)
}

impl EncodeFormat {
    /// The file format this encodes to.
    pub fn file_format(&self) -> FileFormat {
        match self {
            EncodeFormat::Png { .. } => FileFormat::Png,
            EncodeFormat::Jpeg { .. } => FileFormat::Jpeg,
            EncodeFormat::WebP { .. } => FileFormat::WebP,
            EncodeFormat::OpenExr { .. } => FileFormat::OpenExr,
            EncodeFormat::Hdr => FileFormat::Hdr,
            EncodeFormat::Targa => FileFormat::Targa,
            EncodeFormat::Bmp => FileFormat::Bmp,
            EncodeFormat::Tiff { .. } => FileFormat::Tiff,
            EncodeFormat::Iris => FileFormat::Iris,
            EncodeFormat::Dpx { .. } => FileFormat::Dpx,
            EncodeFormat::Cineon => FileFormat::Cineon,
        }
    }
}

/// The default codec: every format of this module.
#[derive(Clone, Copy, Debug, Default)]
pub struct Imbuf;

impl ImageCodec for Imbuf {
    fn decode(&self, bytes: &[u8]) -> Result<DecodedImage, ImageError> {
        decode(bytes)
    }

    fn encode(&self, image: &DecodedImage, format: &EncodeFormat) -> Result<Vec<u8>, ImageError> {
        encode(image, format)
    }
}

/// Decodes any supported format (sniffed from the bytes).
pub fn decode(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
    match FileFormat::detect(bytes) {
        Some(FileFormat::Png) => decode_png(bytes),
        Some(FileFormat::OpenExr) => decode_exr(bytes),
        Some(FileFormat::Ktx2) => crate::ktx2::decode(bytes),
        // 32-bit BMPs carry alpha in OIIO's (and Blender's) reading (WP 5.4).
        Some(FileFormat::Bmp) => crate::encode::decode_bmp_v3_32(bytes)
            .unwrap_or_else(|| decode_with_image(bytes, FileFormat::Bmp)),
        Some(f) => decode_with_image(bytes, f),
        None => Err(ImageError::Unsupported(
            "unrecognised image data (not PNG, JPEG, WebP, OpenEXR, HDR, BMP or TGA)".into(),
        )),
    }
}

/// What [`decode`] would give, read from the file's header without decoding the pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    /// The sniffed format.
    pub format: FileFormat,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Channels [`decode`] gives (1–4).
    pub channels: u8,
    /// Bits per sample [`decode`] gives: 8, 16 or 32 (float).
    pub bit_depth: u8,
}

/// Reads the header of any supported format: the size, channels and sample type [`decode`]
/// would give, without decoding (or allocating) the pixels.
pub fn probe(bytes: &[u8]) -> Result<ImageInfo, ImageError> {
    let format = FileFormat::detect(bytes).ok_or_else(|| {
        ImageError::Unsupported(
            "unrecognised image data (not PNG, JPEG, WebP, OpenEXR, HDR, BMP or TGA)".into(),
        )
    })?;
    if format == FileFormat::Ktx2 {
        return crate::ktx2::probe(bytes);
    }
    let (width, height, channels, bit_depth) = match format {
        FileFormat::Png => {
            let mut decoder = png::Decoder::new(Cursor::new(bytes));
            decoder.set_transformations(png::Transformations::EXPAND);
            let reader = decoder
                .read_info()
                .map_err(|e| ImageError::Decode(format!("PNG: {e}")))?;
            let (ct, bd) = reader.output_color_type();
            let info = reader.info();
            let channels = match ct {
                png::ColorType::Grayscale => 1,
                png::ColorType::GrayscaleAlpha => 2,
                png::ColorType::Rgb => 3,
                png::ColorType::Rgba => 4,
                png::ColorType::Indexed => {
                    return Err(ImageError::Decode("PNG: palette was not expanded".into()));
                }
            };
            let bits = match bd {
                png::BitDepth::Sixteen => 16,
                png::BitDepth::Eight => 8,
                other => return Err(ImageError::Decode(format!("PNG: bit depth {other:?}"))),
            };
            (info.width, info.height, channels, bits)
        }
        FileFormat::OpenExr => {
            let meta = exr::meta::MetaData::read_from_buffered(Cursor::new(bytes), false)
                .map_err(|e| ImageError::Decode(format!("OpenEXR: {e}")))?;
            let header = meta
                .headers
                .iter()
                .find(|h| !h.deep)
                .ok_or_else(|| ImageError::Decode("OpenEXR: no flat layer".into()))?;
            let list = &header.channels.list;
            let has = |n: &str| list.iter().any(|c| c.name.eq_case_insensitive(n));
            // The channel selection of `decode_exr`.
            let colour = if has("R") && has("G") && has("B") {
                3
            } else if has("Y") || has("V") || list.iter().any(|c| !c.name.eq("A")) {
                1
            } else {
                0
            };
            let channels = colour + u8::from(has("A"));
            if channels == 0 {
                return Err(ImageError::Decode("OpenEXR: no channels".into()));
            }
            let size = header.layer_size;
            (size.width() as u32, size.height() as u32, channels, 32)
        }
        FileFormat::Bmp if crate::encode::probe_bmp_v3_32(bytes).is_some() => {
            let (w, h) = crate::encode::probe_bmp_v3_32(bytes).unwrap_or_default();
            (w, h, 4, 8)
        }
        f => {
            use image::ImageDecoder;
            let decoder = image::ImageReader::with_format(Cursor::new(bytes), image_format(f))
                .into_decoder()
                .map_err(|e| ImageError::Decode(format!("{}: {e}", f.identifier())))?;
            let (w, h) = decoder.dimensions();
            // The `DynamicImage` variants of `decode_with_image`.
            let (channels, bits) = match decoder.color_type() {
                image::ColorType::L8 => (1, 8),
                image::ColorType::La8 => (2, 8),
                image::ColorType::Rgb8 => (3, 8),
                image::ColorType::Rgba8 => (4, 8),
                image::ColorType::L16 => (1, 16),
                image::ColorType::La16 => (2, 16),
                image::ColorType::Rgb16 => (3, 16),
                image::ColorType::Rgba16 => (4, 16),
                image::ColorType::Rgb32F => (3, 32),
                _ => (4, 32),
            };
            (w, h, channels, bits)
        }
    };
    Ok(ImageInfo {
        format,
        width,
        height,
        channels,
        bit_depth,
    })
}

/// Encodes to `format`, converting the sample type and channels as the format requires (the
/// per-format encoders live in [`crate::encode`], WP 5.4).
pub fn encode(image: &DecodedImage, format: &EncodeFormat) -> Result<Vec<u8>, ImageError> {
    check(image)?;
    crate::encode::encode(image, format)
}

/// Reads and decodes a file.
pub fn load(path: &Path) -> Result<DecodedImage, ImageError> {
    let bytes =
        std::fs::read(path).map_err(|e| ImageError::Io(format!("{}: {e}", path.display())))?;
    decode(&bytes)
}

/// Encodes and writes a file (the path is used as given; see [`FileFormat::extension`]).
pub fn save(path: &Path, image: &DecodedImage, format: &EncodeFormat) -> Result<(), ImageError> {
    let bytes = encode(image, format)?;
    std::fs::write(path, bytes).map_err(|e| ImageError::Io(format!("{}: {e}", path.display())))
}

pub(crate) fn check(image: &DecodedImage) -> Result<(), ImageError> {
    if !(1..=4).contains(&image.channels) {
        return Err(ImageError::Unsupported(format!(
            "{} channels",
            image.channels
        )));
    }
    let want = image.pixel_count() * image.channels as usize;
    if image.pixels.len() != want {
        return Err(ImageError::Encode(format!(
            "{}x{}x{} image needs {want} samples, buffer has {}",
            image.width,
            image.height,
            image.channels,
            image.pixels.len()
        )));
    }
    if image.width == 0 || image.height == 0 {
        return Err(ImageError::Encode("empty image".into()));
    }
    Ok(())
}

// --- PNG -----------------------------------------------------------------------------------

/// Deflate's largest expansion: 258-byte matches coded in about two bits.
const DEFLATE_MAX_RATIO: u64 = 1032;

fn decode_png(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    // Palette → RGB(A), low-bit grey → 8 bit, tRNS → alpha; 16-bit is kept.
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|e| ImageError::Decode(format!("PNG: {e}")))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| ImageError::Decode("PNG: image too large".into()))?;
    // WP 8.7 (fuzzing): the header alone must not size the allocation. The IDAT stream
    // inflates to `height × (1 + ⌈width × bits per pixel / 8⌉)` bytes (more when interlaced)
    // and Deflate expands at most 1032:1, so a file of `n` bytes holds at most `1032 × n`
    // bytes of rows; a header asking for more is truncated or corrupt.
    {
        let info = reader.info();
        let bpp = u64::from(info.color_type.samples() as u8) * u64::from(info.bit_depth as u8);
        let row = (u64::from(info.width) * bpp).div_ceil(8) + 1;
        let raw = row.saturating_mul(u64::from(info.height));
        if raw > (bytes.len() as u64).saturating_mul(DEFLATE_MAX_RATIO) {
            return Err(ImageError::Decode(format!(
                "PNG: a {}x{} image cannot come from {} bytes (truncated or corrupt file)",
                info.width,
                info.height,
                bytes.len()
            )));
        }
    }
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| ImageError::Decode(format!("PNG: {e}")))?;
    buf.truncate(info.buffer_size());
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(ImageError::Decode("PNG: palette was not expanded".into()));
        }
    };
    let pixels = match info.bit_depth {
        png::BitDepth::Sixteen => PixelData::U16(
            buf.chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect(),
        ),
        png::BitDepth::Eight => PixelData::U8(buf),
        other => return Err(ImageError::Decode(format!("PNG: bit depth {other:?}"))),
    };
    DecodedImage::new(info.width, info.height, channels, pixels)
        .map_err(|e| ImageError::Decode(format!("PNG: {e}")))
}

// --- OpenEXR ---------------------------------------------------------------------------------

/// The channels of an OpenEXR layer: `(name, samples)` in the file's order.
pub type ExrChannels = Vec<(String, Vec<f32>)>;

/// Every channel of the first flat layer of an OpenEXR file, by name (in the file's order),
/// with its samples as floats, and the layer's size: what a multilayer file holds
/// (`ViewLayer.Combined.R`, …), which [`decode`] does not map to RGBA.
pub fn decode_exr_channels(bytes: &[u8]) -> Result<(u32, u32, ExrChannels), ImageError> {
    exr_check_size(bytes)?;
    use exr::prelude::FlatSamples;
    use exr::prelude::traits::*;
    let img = read()
        .no_deep_data()
        .largest_resolution_level()
        .all_channels()
        .first_valid_layer()
        .all_attributes()
        .non_parallel()
        .from_buffered(Cursor::new(bytes))
        .map_err(|e| ImageError::Decode(format!("OpenEXR: {e}")))?;
    let layer = img.layer_data;
    let channels = layer
        .channel_data
        .list
        .iter()
        .map(|c| {
            let v: Vec<f32> = match &c.sample_data {
                FlatSamples::F16(v) => v.iter().map(|x| x.to_f32()).collect(),
                FlatSamples::F32(v) => v.clone(),
                FlatSamples::U32(v) => v.iter().map(|&x| x as f32).collect(),
            };
            (c.name.to_string(), v)
        })
        .collect();
    Ok((
        layer.size.width() as u32,
        layer.size.height() as u32,
        channels,
    ))
}

/// WP 8.7 (fuzzing): refuses an OpenEXR whose header declares more sample data than its
/// bytes can hold, before the `exr` crate allocates it from the header alone, and reserves
/// the decoded buffer in the statement's budget (`--max-elements`,
/// [`frap_math::budget::reserve_buffer`]). Below 64 MiB every header is accepted; above, the
/// bound is the largest expansion of the file's codecs ([`exr_max_ratio`]), so only a corrupt
/// file is refused.
fn exr_check_size(bytes: &[u8]) -> Result<(), ImageError> {
    const FLOOR: u64 = 64 << 20;
    let Ok(meta) = exr::meta::MetaData::read_from_buffered(Cursor::new(bytes), false) else {
        // The reader reports the error itself.
        return Ok(());
    };
    let (mut total, mut decoded, mut ratio): (u64, u64, u64) = (0, 0, 1);
    for h in meta.headers.iter().filter(|h| !h.deep) {
        let (w, hh) = (h.layer_size.width() as u64, h.layer_size.height() as u64);
        ratio = ratio.max(exr_max_ratio(h.compression));
        for c in &h.channels.list {
            let size = match c.sample_type {
                exr::meta::attribute::SampleType::F16 => 2,
                _ => 4,
            };
            let n = w.saturating_mul(hh);
            total = total.saturating_add(n.saturating_mul(size));
            decoded = decoded.saturating_add(n.saturating_mul(4));
        }
    }
    if total > FLOOR.max((bytes.len() as u64).saturating_mul(ratio)) {
        return Err(ImageError::Decode(format!(
            "OpenEXR: {total} bytes of samples cannot come from {} bytes (truncated or corrupt file)",
            bytes.len()
        )));
    }
    frap_math::budget::reserve_buffer("OpenEXR image", Some(decoded))
        .map_err(|e| ImageError::Decode(format!("OpenEXR: {e}")))?;
    let dwa = |h: &exr::meta::header::Header| {
        matches!(
            h.compression,
            exr::compression::Compression::DWAA(_) | exr::compression::Compression::DWAB(_)
        )
    };
    if meta.headers.iter().any(dwa) {
        exr_check_dwa(bytes)?;
    }
    Ok(())
}

/// The largest ratio of sample bytes to file bytes a codec reaches (on a flat image):
/// uncompressed 1; RLE 64 (a 2-byte run codes 128 bytes); ZIP, ZIPS and PIZ (Huffman with
/// 255-symbol run codes, about 400:1) at most Deflate's 1032; PXR24 (32-bit floats cut to
/// 24 bits before Deflate) 4/3 of that; B44/B44A 11 (a flat 4×4 block of halves in 3 bytes);
/// DWAA/DWAB 64 × 1032 (a flat 8×8 block of halves becomes one 2-byte DC value and an
/// end-of-block code, both Deflated again: Blender 5.2.4 writes a flat 4096² RGBA DWAA in
/// 33,969 bytes, 3951:1); HTJ2K as DWA (not bounded more tightly).
fn exr_max_ratio(c: exr::compression::Compression) -> u64 {
    use exr::compression::Compression as C;
    match c {
        C::Uncompressed => 1,
        C::RLE => 64,
        C::ZIP1 | C::ZIP16 | C::PIZ => DEFLATE_MAX_RATIO,
        C::PXR24 => DEFLATE_MAX_RATIO * 4 / 3 + 1,
        C::B44 | C::B44A => 11,
        _ => 64 * DEFLATE_MAX_RATIO,
    }
}

/// WP 8.7 (fuzzing): `exr` 1.74.3's DWA decoder sizes its zlib output buffers from the
/// counters at the start of each chunk (`DataSizesSingle`) before inflating, so a corrupt
/// counter asks for terabytes. Each counter must fit the chunk: at most four times the
/// 4-byte samples of a chunk of the layer's width and up to 256 lines (DWAB), padded to 8×8
/// DCT blocks.
fn exr_check_dwa(bytes: &[u8]) -> Result<(), ImageError> {
    use exr::block::chunk::CompressedBlock;
    let corrupt = |what: &str| ImageError::Decode(format!("OpenEXR: corrupt DWA chunk ({what})"));
    let Ok(reader) = exr::block::read(Cursor::new(bytes), false) else {
        return Ok(());
    };
    let headers = reader.meta_data().headers.clone();
    let Ok(chunks) = reader.all_chunks(false) else {
        return Ok(());
    };
    for chunk in chunks {
        let Ok(chunk) = chunk else {
            return Ok(());
        };
        let Some(h) = headers.get(chunk.layer_index) else {
            continue;
        };
        // A scan-line chunk holds at most 256 lines (DWAB); a tile at most the layer.
        let (data, max_lines) = match &chunk.compressed_block {
            CompressedBlock::ScanLine(b) => (&b.compressed_pixels_le, 256),
            CompressedBlock::Tile(b) => (&b.compressed_pixels_le, u64::MAX),
            _ => continue,
        };
        let (w, lines) = (
            h.layer_size.width() as u64,
            (h.layer_size.height() as u64).min(max_lines),
        );
        let channels = h.channels.list.len() as u64;
        let limit = 16u64
            .saturating_mul(w + 8)
            .saturating_mul(lines + 8)
            .saturating_mul(channels.max(1));
        // Counters 1..=9 (sizes and the AC/DC counts, the latter in 2-byte units).
        for (i, c) in data.chunks_exact(8).skip(1).take(9).enumerate() {
            let v = u64::from_le_bytes(c.try_into().expect("invariant: 8-byte chunk"));
            let bytes_needed = if i >= 7 { v.saturating_mul(2) } else { v };
            if bytes_needed > limit {
                return Err(corrupt("a size counter exceeds the chunk"));
            }
        }
    }
    Ok(())
}

fn decode_exr(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
    exr_check_size(bytes)?;
    use exr::prelude::traits::*;
    use exr::prelude::{AnyChannel, FlatSamples, Vec2};
    let img = read()
        .no_deep_data()
        .largest_resolution_level()
        .all_channels()
        .first_valid_layer()
        .all_attributes()
        .non_parallel()
        .from_buffered(Cursor::new(bytes))
        .map_err(|e| ImageError::Decode(format!("OpenEXR: {e}")))?;
    let layer = img.layer_data;
    let (w, h) = (layer.size.width(), layer.size.height());
    let n = w * h;
    let find = |name: &str| {
        layer
            .channel_data
            .list
            .iter()
            .find(|c| c.name.eq_case_insensitive(name))
    };
    let samples = |c: &AnyChannel<FlatSamples>| -> Result<Vec<f32>, ImageError> {
        if c.sampling != Vec2(1, 1) {
            return Err(ImageError::Unsupported(
                "OpenEXR: subsampled channels".into(),
            ));
        }
        let v: Vec<f32> = match &c.sample_data {
            FlatSamples::F16(v) => v.iter().map(|x| x.to_f32()).collect(),
            FlatSamples::F32(v) => v.clone(),
            FlatSamples::U32(v) => v.iter().map(|&x| x as f32).collect(),
        };
        if v.len() != n {
            return Err(ImageError::Decode("OpenEXR: channel size mismatch".into()));
        }
        Ok(v)
    };
    let mut planes: Vec<Vec<f32>> = Vec::new();
    if let (Some(r), Some(g), Some(b)) = (find("R"), find("G"), find("B")) {
        planes.push(samples(r)?);
        planes.push(samples(g)?);
        planes.push(samples(b)?);
    } else if let Some(y) = find("Y").or_else(|| find("V")) {
        planes.push(samples(y)?);
    } else if let Some(first) = layer.channel_data.list.iter().find(|c| !c.name.eq("A")) {
        planes.push(samples(first)?);
    }
    if let Some(a) = find("A") {
        planes.push(samples(a)?);
    }
    if planes.is_empty() {
        return Err(ImageError::Decode("OpenEXR: no channels".into()));
    }
    let channels = planes.len();
    let mut data = Vec::with_capacity(n * channels);
    for i in 0..n {
        for p in &planes {
            data.push(p[i]);
        }
    }
    DecodedImage::new(w as u32, h as u32, channels as u8, PixelData::F32(data))
        .map_err(|e| ImageError::Decode(format!("OpenEXR: {e}")))
}

// --- formats through the `image` crate -------------------------------------------------------

fn image_format(f: FileFormat) -> image::ImageFormat {
    match f {
        FileFormat::Png => image::ImageFormat::Png,
        FileFormat::Jpeg => image::ImageFormat::Jpeg,
        FileFormat::WebP => image::ImageFormat::WebP,
        FileFormat::OpenExr => image::ImageFormat::OpenExr,
        FileFormat::Hdr => image::ImageFormat::Hdr,
        FileFormat::Targa => image::ImageFormat::Tga,
        FileFormat::Bmp => image::ImageFormat::Bmp,
        // Never sniffed (`FileFormat::detect`), or decoded by `ktx2.rs`: never through `image`.
        FileFormat::Tiff
        | FileFormat::Iris
        | FileFormat::Dpx
        | FileFormat::Cineon
        | FileFormat::Ktx2 => image::ImageFormat::Tiff,
    }
}

fn decode_with_image(bytes: &[u8], format: FileFormat) -> Result<DecodedImage, ImageError> {
    use image::DynamicImage as D;
    let img = image::load_from_memory_with_format(bytes, image_format(format))
        .map_err(|e| ImageError::Decode(format!("{}: {e}", format.identifier())))?;
    let (w, h) = (img.width(), img.height());
    let (channels, pixels) = match img {
        D::ImageLuma8(b) => (1, PixelData::U8(b.into_raw())),
        D::ImageLumaA8(b) => (2, PixelData::U8(b.into_raw())),
        D::ImageRgb8(b) => (3, PixelData::U8(b.into_raw())),
        D::ImageRgba8(b) => (4, PixelData::U8(b.into_raw())),
        D::ImageLuma16(b) => (1, PixelData::U16(b.into_raw())),
        D::ImageLumaA16(b) => (2, PixelData::U16(b.into_raw())),
        D::ImageRgb16(b) => (3, PixelData::U16(b.into_raw())),
        D::ImageRgba16(b) => (4, PixelData::U16(b.into_raw())),
        D::ImageRgb32F(b) => (3, PixelData::F32(b.into_raw())),
        D::ImageRgba32F(b) => (4, PixelData::F32(b.into_raw())),
        other => (4, PixelData::F32(other.to_rgba32f().into_raw())),
    };
    DecodedImage::new(w, h, channels, pixels)
        .map_err(|e| ImageError::Decode(format!("{}: {e}", format.identifier())))
}
