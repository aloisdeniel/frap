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

/// Encodes to `format`, converting the sample type and channels as the format requires.
pub fn encode(image: &DecodedImage, format: &EncodeFormat) -> Result<Vec<u8>, ImageError> {
    check(image)?;
    match format {
        EncodeFormat::Png {
            bit_depth,
            compression,
        } => encode_png(image, *bit_depth, *compression),
        EncodeFormat::Jpeg { quality } => encode_jpeg(image, *quality),
        EncodeFormat::WebP { lossless, quality } => encode_webp(image, *lossless, *quality),
        EncodeFormat::OpenExr { bit_depth } => encode_exr(image, *bit_depth),
        EncodeFormat::Hdr => encode_hdr(image),
        EncodeFormat::Targa => encode_8bit(image, image::ImageFormat::Tga),
        EncodeFormat::Bmp => encode_8bit(image, image::ImageFormat::Bmp),
    }
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

fn check(image: &DecodedImage) -> Result<(), ImageError> {
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

/// Blender's PNG `compression` percent → zlib level (`format_png.cc`:
/// `int(float(compress) / 11.1111f)` clamped to 0–9).
pub fn png_zlib_level(compression: u8) -> u8 {
    ((f32::from(compression) / 11.1111) as i32).clamp(0, 9) as u8
}

fn encode_png(image: &DecodedImage, bit_depth: u8, compression: u8) -> Result<Vec<u8>, ImageError> {
    let color = match image.channels {
        1 => png::ColorType::Grayscale,
        2 => png::ColorType::GrayscaleAlpha,
        3 => png::ColorType::Rgb,
        _ => png::ColorType::Rgba,
    };
    let data: Vec<u8> = match bit_depth {
        8 => image.to_u8(),
        16 => image
            .to_u16()
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect(),
        other => {
            return Err(ImageError::Unsupported(format!("PNG bit depth {other}")));
        }
    };
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, image.width, image.height);
        enc.set_color(color);
        enc.set_depth(if bit_depth == 16 {
            png::BitDepth::Sixteen
        } else {
            png::BitDepth::Eight
        });
        let level = png_zlib_level(compression);
        enc.set_deflate_compression(if level == 0 {
            png::DeflateCompression::NoCompression
        } else {
            png::DeflateCompression::Level(level)
        });
        let mut writer = enc
            .write_header()
            .map_err(|e| ImageError::Encode(format!("PNG: {e}")))?;
        writer
            .write_image_data(&data)
            .map_err(|e| ImageError::Encode(format!("PNG: {e}")))?;
        writer
            .finish()
            .map_err(|e| ImageError::Encode(format!("PNG: {e}")))?;
    }
    Ok(out)
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

fn encode_exr(image: &DecodedImage, bit_depth: u8) -> Result<Vec<u8>, ImageError> {
    use exr::prelude::{
        AnyChannel, AnyChannels, Encoding, FlatSamples, Image, Layer, LayerAttributes,
        WritableImage, f16,
    };
    if bit_depth != 16 && bit_depth != 32 {
        return Err(ImageError::Unsupported(format!(
            "OpenEXR bit depth {bit_depth}"
        )));
    }
    let names: &[&str] = match image.channels {
        1 => &["Y"],
        2 => &["Y", "A"],
        3 => &["R", "G", "B"],
        _ => &["R", "G", "B", "A"],
    };
    let data = image.to_f32();
    let c = image.channels as usize;
    let list = names
        .iter()
        .enumerate()
        .map(|(k, name)| {
            let plane = data.iter().skip(k).step_by(c).copied();
            let samples = if bit_depth == 16 {
                FlatSamples::F16(plane.map(f16::from_f32).collect())
            } else {
                FlatSamples::F32(plane.collect())
            };
            AnyChannel::new(*name, samples)
        })
        .collect();
    let layer = Layer::new(
        (image.width as usize, image.height as usize),
        LayerAttributes::default(),
        Encoding::SMALL_LOSSLESS,
        AnyChannels::sort(list),
    );
    let mut out = Cursor::new(Vec::new());
    Image::from_layer(layer)
        .write()
        .non_parallel()
        .to_buffered(&mut out)
        .map_err(|e| ImageError::Encode(format!("OpenEXR: {e}")))?;
    Ok(out.into_inner())
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

fn color_type_8(channels: u8) -> image::ExtendedColorType {
    match channels {
        1 => image::ExtendedColorType::L8,
        2 => image::ExtendedColorType::La8,
        3 => image::ExtendedColorType::Rgb8,
        _ => image::ExtendedColorType::Rgba8,
    }
}

fn encode_8bit(image: &DecodedImage, format: image::ImageFormat) -> Result<Vec<u8>, ImageError> {
    use image::ImageEncoder;
    let data = image.to_u8();
    let mut out = Vec::new();
    let ct = color_type_8(image.channels);
    let res = match format {
        image::ImageFormat::Tga => image::codecs::tga::TgaEncoder::new(&mut out).write_image(
            &data,
            image.width,
            image.height,
            ct,
        ),
        _ => image::codecs::bmp::BmpEncoder::new(&mut out).write_image(
            &data,
            image.width,
            image.height,
            ct,
        ),
    };
    res.map_err(|e| ImageError::Encode(format!("{format:?}: {e}")))?;
    Ok(out)
}

fn encode_jpeg(image: &DecodedImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    use image::ImageEncoder;
    // JPEG has no alpha: BW+alpha → BW, RGBA → RGB (as Blender, which offers BW/RGB only).
    let img = match image.channels {
        2 => image.with_channels(1)?,
        4 => image.with_channels(3)?,
        _ => image.clone(),
    };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
        .write_image(
            &img.to_u8(),
            img.width,
            img.height,
            color_type_8(img.channels),
        )
        .map_err(|e| ImageError::Encode(format!("JPEG: {e}")))?;
    Ok(out)
}

fn encode_webp(image: &DecodedImage, lossless: bool, quality: u8) -> Result<Vec<u8>, ImageError> {
    use image::ImageEncoder;
    if !lossless {
        return encode_webp_lossy(image, quality);
    }
    let mut out = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut out)
        .write_image(
            &image.to_u8(),
            image.width,
            image.height,
            color_type_8(image.channels),
        )
        .map_err(|e| ImageError::Encode(format!("WEBP: {e}")))?;
    Ok(out)
}

#[cfg(feature = "webp-lossy")]
fn encode_webp_lossy(image: &DecodedImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    let img = match image.channels {
        1 => image.with_channels(3)?,
        2 => image.with_channels(4)?,
        _ => image.clone(),
    };
    let data = img.to_u8();
    let enc = if img.channels == 4 {
        webp::Encoder::from_rgba(&data, img.width, img.height)
    } else {
        webp::Encoder::from_rgb(&data, img.width, img.height)
    };
    let mem = enc
        .encode_simple(false, f32::from(quality.min(100)))
        .map_err(|e| ImageError::Encode(format!("WEBP: {e:?}")))?;
    Ok(mem.to_vec())
}

#[cfg(not(feature = "webp-lossy"))]
fn encode_webp_lossy(_image: &DecodedImage, _quality: u8) -> Result<Vec<u8>, ImageError> {
    Err(ImageError::Unsupported(
        "lossy WebP encoding needs frap built with the `webp-lossy` feature (lossless WebP \
         is always available)"
            .into(),
    ))
}

fn encode_hdr(image: &DecodedImage) -> Result<Vec<u8>, ImageError> {
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
