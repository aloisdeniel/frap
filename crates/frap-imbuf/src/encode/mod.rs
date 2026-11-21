// SPDX-License-Identifier: GPL-3.0-or-later
//! Encoders per output format (ARCHITECTURE §19.4: PNG 8/16, JPEG, WEBP, OPEN_EXR, HDR, TARGA,
//! BMP; TIFF, IRIS, DPX, CINEON and multilayer OpenEXR since the M5+M6 stabilization). Owned by WP 5.4 (`crates/frap-imbuf/src/encode/**`, reassigned in M5; the M5+M6 W0
//! integrator moved the M1 encoders here from `formats.rs`). [`encode`] is called by
//! `formats::encode` after the buffer check, so every encoder receives a consistent image.
//!
//! Blender 5.2.4 writes PNG, Targa, BMP, Radiance HDR and WebP through OpenImageIO 3.1, JPEG
//! through libjpeg-turbo 2.1 (`imbuf/intern/format_jpeg.cc`) and OpenEXR through OpenEXR 3.4
//! (`imbuf/intern/openexr/openexr_api.cpp`). The writers here follow them:
//!
//! | Format | Writer | Same as Blender's |
//! |---|---|---|
//! | Targa | port of OIIO's `targaoutput.cpp` (RLE or raw, TGA 2.0 footer) | bytes |
//! | BMP | port of OIIO's `bmpoutput.cpp` (V3 header, grey palette, BGR(A)) | bytes |
//! | Radiance HDR | port of OIIO's `hdroutput.cpp` (`float2rgbe`, per-channel RLE) | bytes |
//! | JPEG | port of libjpeg-turbo's baseline path with Blender's settings (float DCT, 4:2:0) | bytes, but the stamp `COM` markers |
//! | PNG | `png` crate (other deflate stream), OIIO's colour and resolution chunks | samples |
//! | WebP | `image` lossless / libwebp lossy (`webp-lossy` feature) | samples (lossless) |
//! | OpenEXR | `exr` crate, Blender's codecs, channel names and half clamping | samples |
//! | TIFF | frap's baseline writer (OIIO's tags, strips, predictor, alpha kind) | samples |
//! | IRIS | port of OIIO's `sgioutput.cpp` (RLE) | bytes |
//! | DPX | port of OIIO's `dpxoutput.cpp` / libdpx (header, packing) | image data bytes |
//! | Cineon | port of Blender's `cineonlib.cc` / `logImageCore.cc` | image data bytes |
//!
//! [`WriteOptions`] carries what [`EncodeFormat`] does not (Targa compression, the OpenEXR
//! codec, the render's pixels per metre, the colour space of display-referred samples);
//! [`encode`] uses [`WriteOptions::default`], Blender's defaults for an image save.

mod bmp;
mod cineon;
mod dpx;
mod exr;
mod hdr;
mod jpeg;
mod png;
mod raw8;
mod sgi;
mod tga;
mod tiff;
mod webp;

#[cfg(test)]
mod tests;

pub use bmp::{decode_bmp_v3_32, probe_bmp_v3_32};
pub use cineon::{decode_cineon, is_cineon};
pub use dpx::{DpxSettings, decode_dpx, is_dpx};
pub use png::png_zlib_level;
pub use sgi::{decode_sgi, is_sgi};
pub use tiff::{TiffCodec, decode_tiff, tiff_extra_samples};
/// The deadline-bound worker of the WebP encoders, shared with the Basis encoder (WP 7.3).
#[cfg_attr(not(feature = "basisu-encode"), allow(unused_imports))]
pub(crate) use webp::cancellable_with;

use crate::codec::{DecodedImage, EncodeFormat, ImageError};

/// `image_settings.exr_codec` (Blender's `R_IMF_EXR_CODEC_*`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ExrCodec {
    /// `'NONE'`
    None,
    /// `'ZIP'`: zlib, 16 scanlines per block (Blender's default).
    #[default]
    Zip,
    /// `'ZIPS'`: zlib, one scanline per block.
    Zips,
    /// `'PIZ'`: wavelet.
    Piz,
    /// `'RLE'`
    Rle,
    /// `'PXR24'`: float samples rounded to 24 bits (lossy for 32-bit).
    Pxr24,
    /// `'B44'`: lossy for half samples.
    B44,
    /// `'B44A'`: B44 with flat areas packed.
    B44a,
    /// `'DWAA'` (lossy, 32 scanlines) at `image_settings.quality`.
    Dwaa(u8),
    /// `'DWAB'` (lossy, 256 scanlines) at `image_settings.quality`.
    Dwab(u8),
}

impl ExrCodec {
    /// Parses `exr_codec` (`quality` is `image_settings.quality`, used by DWAA and DWAB).
    /// `None` for codecs frap cannot write (`'HTJ2K'`).
    pub fn from_identifier(id: &str, quality: u8) -> Option<ExrCodec> {
        Some(match id {
            "NONE" => ExrCodec::None,
            "ZIP" => ExrCodec::Zip,
            "ZIPS" => ExrCodec::Zips,
            "PIZ" => ExrCodec::Piz,
            "RLE" => ExrCodec::Rle,
            "PXR24" => ExrCodec::Pxr24,
            "B44" => ExrCodec::B44,
            "B44A" => ExrCodec::B44a,
            "DWAA" => ExrCodec::Dwaa(quality),
            "DWAB" => ExrCodec::Dwab(quality),
            _ => return None,
        })
    }
}

/// Writer settings beyond [`EncodeFormat`] (Blender's `ImBuf.foptions`, `ppm` and colour space).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct WriteOptions {
    /// Targa: run-length encoded (`'TARGA'`, default) or raw (`'TARGA_RAW'`).
    pub targa_rle: bool,
    /// OpenEXR compression.
    pub exr_codec: ExrCodec,
    /// OpenEXR name of the single channel of a one-channel image: `"Y"` (default) or `"V"`, which
    /// Blender's render output uses for `color_mode='BW'`.
    pub exr_grey_channel: &'static str,
    /// OpenEXR `colorInteropID` attribute (`"lin_rec709_scene"` for Blender's scene-linear
    /// render output).
    pub exr_interop_id: Option<&'static str>,
    /// Pixels per metre (render `ppm`: `ppm_factor / ppm_base`): BMP resolution, PNG `pHYs`,
    /// OpenEXR `xDensity` and `pixelAspectRatio`, as Blender writes them.
    pub ppm: Option<[f64; 2]>,
    /// The samples are sRGB display-encoded: PNG gets `sRGB`, `gAMA` and `cHRM` chunks (OIIO's
    /// `png_set_sRGB_gAMA_and_cHRM` for `srgb_rec709_scene`).
    pub srgb: bool,
    /// TIFF: the alpha of a 4-channel file is associated (`ExtraSamples` 1; Blender's 16-bit
    /// RGBA) rather than unassociated (2).
    pub tiff_associated_alpha: bool,
    /// Multilayer OpenEXR: the part (`"ViewLayer.Combined"`) whose name prefixes the channel
    /// names (`"ViewLayer.Combined.R"`), as Blender's `OPEN_EXR_MULTILAYER` render output
    /// writes the Combined pass.
    pub exr_layer: Option<String>,
    /// DPX / Cineon: the file name the header records (Blender writes the output's name).
    pub file_name: String,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            targa_rle: true,
            exr_codec: ExrCodec::Zip,
            exr_grey_channel: "Y",
            exr_interop_id: None,
            ppm: None,
            srgb: false,
            tiff_associated_alpha: false,
            exr_layer: None,
            file_name: String::new(),
        }
    }
}

/// Encodes a checked image (`formats::encode` validated channels and buffer size) with the
/// default [`WriteOptions`].
pub(crate) fn encode(image: &DecodedImage, format: &EncodeFormat) -> Result<Vec<u8>, ImageError> {
    encode_checked(image, format, &WriteOptions::default())
}

/// Encodes to `format` with `options` (checks the image first, like [`crate::encode`]).
pub fn encode_with(
    image: &DecodedImage,
    format: &EncodeFormat,
    options: &WriteOptions,
) -> Result<Vec<u8>, ImageError> {
    crate::formats::check(image)?;
    encode_checked(image, format, options)
}

fn encode_checked(
    image: &DecodedImage,
    format: &EncodeFormat,
    options: &WriteOptions,
) -> Result<Vec<u8>, ImageError> {
    match format {
        EncodeFormat::Png {
            bit_depth,
            compression,
        } => png::encode_png(image, *bit_depth, *compression, options),
        EncodeFormat::Jpeg { quality } => jpeg::encode_jpeg(image, *quality),
        EncodeFormat::WebP { lossless, quality } => webp::encode_webp(image, *lossless, *quality),
        EncodeFormat::OpenExr { bit_depth } => exr::encode_exr(image, *bit_depth, options),
        EncodeFormat::Hdr => hdr::encode_hdr(image),
        EncodeFormat::Targa => tga::encode_tga(image, options.targa_rle),
        EncodeFormat::Bmp => bmp::encode_bmp(image, options.ppm),
        EncodeFormat::Tiff { bit_depth, codec } => {
            let image = to_bits(image, *bit_depth)?;
            tiff::encode_tiff(&image, *codec, options)
        }
        EncodeFormat::Iris => sgi::encode_sgi(&*to_bits(image, 8)?),
        EncodeFormat::Dpx { bit_depth, log } => {
            let image = to_bits(image, if *bit_depth == 8 { 8 } else { 16 })?;
            let s = DpxSettings {
                bits: *bit_depth,
                log: *log,
            };
            dpx::encode_dpx(&image, s, &options.file_name)
        }
        EncodeFormat::Cineon => cineon::encode_cineon(image, &options.file_name),
    }
}

/// `image` with 8- or 16-bit samples (converted with Blender's rounding when the type differs).
fn to_bits(
    image: &DecodedImage,
    bits: u8,
) -> Result<std::borrow::Cow<'_, DecodedImage>, ImageError> {
    use crate::codec::PixelData;
    use std::borrow::Cow;
    Ok(match (&image.pixels, bits) {
        (PixelData::U8(_), 8) | (PixelData::U16(_), 16) => Cow::Borrowed(image),
        _ => {
            let f = image.to_f32();
            let pixels = if bits == 8 {
                PixelData::U8(f.iter().map(|&v| crate::convert::f32_to_u8(v)).collect())
            } else {
                PixelData::U16(f.iter().map(|&v| crate::convert::f32_to_u16(v)).collect())
            };
            Cow::Owned(DecodedImage::new(
                image.width,
                image.height,
                image.channels,
                pixels,
            )?)
        }
    })
}
