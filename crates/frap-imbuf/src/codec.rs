// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoded images and the codec trait (consumed by `frap-render-scene`, `frap-io-gltf`,
//! `frap-ops-render`; types frozen at M1 W0 because `RenderScene.images` names them).

/// Pixel storage, row-major, top row first, `channels` interleaved.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PixelData {
    /// 8-bit.
    U8(Vec<u8>),
    /// 16-bit.
    U16(Vec<u16>),
    /// 32-bit float (HDR, EXR, render results).
    F32(Vec<f32>),
}

/// A decoded image.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Channels per pixel (1–4).
    pub channels: u8,
    /// Pixels.
    pub pixels: PixelData,
}

/// Output format and options (`image_settings.file_format/color_depth/compression/quality`).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum EncodeFormat {
    /// PNG, 8 or 16 bit, zlib `compression` 0–100.
    Png {
        /// 8 | 16
        bit_depth: u8,
        /// `compression` percent
        compression: u8,
    },
    /// JPEG with `quality`.
    Jpeg {
        /// 0–100
        quality: u8,
    },
    /// WebP; lossy needs the `webp-lossy` feature.
    WebP {
        /// Lossless encode.
        lossless: bool,
        /// 0–100
        quality: u8,
    },
    /// OpenEXR (half or float).
    OpenExr {
        /// 16 | 32
        bit_depth: u8,
    },
    /// Radiance HDR.
    Hdr,
    /// Targa.
    Targa,
    /// BMP.
    Bmp,
}

/// Image I/O failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ImageError {
    /// Not a supported format.
    #[error("unsupported image format: {0}")]
    Unsupported(String),
    /// Corrupt or truncated data.
    #[error("cannot decode image: {0}")]
    Decode(String),
    /// Encoder failure.
    #[error("cannot encode image: {0}")]
    Encode(String),
    /// File read/write failure (`load`/`save`).
    #[error("{0}")]
    Io(String),
}

/// Decoder/encoder (one implementation per format family, `formats`).
pub trait ImageCodec {
    /// Decodes encoded bytes.
    fn decode(&self, bytes: &[u8]) -> Result<DecodedImage, ImageError>;
    /// Encodes.
    fn encode(&self, image: &DecodedImage, format: &EncodeFormat) -> Result<Vec<u8>, ImageError>;
}
