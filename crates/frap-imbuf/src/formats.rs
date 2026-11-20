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

fn decode_exr(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
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
