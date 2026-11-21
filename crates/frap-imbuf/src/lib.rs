// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-imbuf` (layer L3): image decode/encode behind `ImageCodec`: PNG 8/16, JPEG, WebP
//! (lossless pure Rust; lossy via the `webp-lossy` feature), HDR, EXR, TGA, BMP; KTX2 read and
//! Basis transcode (feature `basisu`) and Basis encode (`basisu-encode`) arrive in M7 (WP 7.3;
//! [`ktx2`], [`basis`]; ARCHITECTURE §3.2).
//!
//! [`decode`] sniffs the format and keeps the file's sample type; [`encode`] converts to what
//! the target stores with Blender's rounding ([`convert`]). [`Imbuf`] is the `ImageCodec`.
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.6. M5+M6 W0: the encoders moved to
//! [`encode`] (WP 5.4, `encode/**`); `formats::png_zlib_level` stays a re-export.
#![deny(unsafe_code)]

/// Basis Universal transcode / encode (WP 7.3; M7 W0 stub).
pub mod basis;
pub mod codec;
pub mod convert;
pub mod encode;
pub mod formats;
/// KTX2 container read (WP 7.3; M7 W0 stub).
pub mod ktx2;

pub use codec::{DecodedImage, EncodeFormat, ImageCodec, ImageError, PixelData};
pub use formats::{
    FileFormat, ImageInfo, Imbuf, decode, decode_exr_channels, encode, load, probe, save,
};
