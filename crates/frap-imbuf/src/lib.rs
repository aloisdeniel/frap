// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-imbuf` (layer L3): image decode/encode behind `ImageCodec`: PNG 8/16, JPEG, WebP
//! (lossless pure Rust; lossy via the `webp-lossy` feature), HDR, EXR, TGA, BMP; KTX2 + Basis
//! arrive in M3 (ARCHITECTURE §3.2).
//!
//! [`decode`] sniffs the format and keeps the file's sample type; [`encode`] converts to what
//! the target stores with Blender's rounding ([`convert`]). [`Imbuf`] is the `ImageCodec`.
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.6.
#![deny(unsafe_code)]

pub mod codec;
pub mod convert;
pub mod formats;

pub use codec::{DecodedImage, EncodeFormat, ImageCodec, ImageError, PixelData};
pub use formats::{FileFormat, ImageInfo, Imbuf, decode, encode, load, probe, save};
