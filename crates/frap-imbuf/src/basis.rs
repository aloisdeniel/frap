// SPDX-License-Identifier: GPL-3.0-or-later
//! Basis Universal (WP 7.3, decision log M7W0-7): transcoding of ETC1S / UASTC payloads to
//! RGBA (the pure-Rust `basisu` crate, a bit-exact port of the reference transcoder; feature
//! `basisu`) for [`crate::ktx2`], and encoding (the reference C++ encoder through
//! `basisu_c_sys`, feature `basisu-encode`, off by default: it needs libclang) for
//! `export_scene.gltf(frap_texture_format='KTX2')`.
//!
//! The encoder writes a KTX2 file: ETC1S with BasisLZ supercompression, or UASTC LDR 4x4 with
//! Zstandard supercompression; the DFD transfer function is sRGB for colour data and linear
//! otherwise ([`BasisOptions::srgb`]); a full mip chain is generated (clamped or wrapped at
//! the borders as the texture's sampler repeats) because glTF samplers default to mipmapped
//! minification and a GPU cannot build mips of a block-compressed texture. Rows go in top
//! row first, as `KHR_texture_basisu` requires (`KTXorientation` `rd`).
//!
//! Two more rules of `KHR_texture_basisu` hold for every file written here:
//!
//! - `pixelWidth` and `pixelHeight` MUST be multiples of 4: an image of another size is first
//!   resampled (bilinear, on the stored 8-bit samples) to the next multiple of 4 in each
//!   dimension ([`ktx2_size`]; a 5x3 image is written 8x4, a 1x1 image 4x4). frap rounds up
//!   to a multiple of 4, not to a power of two as `toktx --resize` or `gltfpack -tp` can: the
//!   smallest change the specification allows, and the aspect ratio stays close;
//! - non-colour data MUST have the colour primaries `KHR_DF_PRIMARIES_UNSPECIFIED`: the
//!   encoder writes BT709 whatever the transfer function, so the DFD byte is set to 0 after
//!   encoding when [`BasisOptions::srgb`] is false (colour keeps BT709 and sRGB).
//!
//! `--stmt-timeout`: the encoder does not poll frap's budget, so under a deadline it runs on
//! a detached worker like the WebP encoders (`encode/webp.rs`, at most two at once).

use crate::codec::{DecodedImage, ImageError};

/// `frap_basisu_mode`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum BasisMode {
    /// `ETC1S`: smaller files, lower quality.
    #[default]
    Etc1s,
    /// `UASTC`: larger files, higher quality.
    Uastc,
}

impl BasisMode {
    /// Parses the `frap_basisu_mode` identifier.
    pub fn from_identifier(id: &str) -> Option<BasisMode> {
        match id {
            "ETC1S" => Some(BasisMode::Etc1s),
            "UASTC" => Some(BasisMode::Uastc),
            _ => None,
        }
    }
}

/// Whether this build can encode Basis Universal KTX2 files (feature `basisu-encode`).
pub const CAN_ENCODE: bool = cfg!(feature = "basisu-encode");

/// Encoder settings beyond the mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct BasisOptions {
    /// ETC1S or UASTC.
    pub mode: BasisMode,
    /// The samples are sRGB-encoded colour (DFD transfer sRGB); `false`: linear data.
    pub srgb: bool,
    /// Generate the mip chain.
    pub mipmaps: bool,
    /// Mips wrap at the borders (a repeating sampler) instead of clamping.
    pub wrap: bool,
    /// The encoder's unified quality, 1–100.
    pub quality: u8,
}

impl BasisOptions {
    /// The defaults for `mode`: sRGB colour, wrapped mips, quality 75.
    pub fn new(mode: BasisMode) -> BasisOptions {
        BasisOptions {
            mode,
            srgb: true,
            mipmaps: true,
            wrap: true,
            quality: 75,
        }
    }

    /// With the transfer function given.
    pub fn with_srgb(self, srgb: bool) -> BasisOptions {
        BasisOptions { srgb, ..self }
    }

    /// With mips wrapped (`true`) or clamped at the borders.
    pub fn with_wrap(self, wrap: bool) -> BasisOptions {
        BasisOptions { wrap, ..self }
    }

    /// With or without the mip chain.
    pub fn with_mipmaps(self, mipmaps: bool) -> BasisOptions {
        BasisOptions { mipmaps, ..self }
    }

    /// With the unified quality (clamped to 1–100).
    pub fn with_quality(self, quality: u8) -> BasisOptions {
        BasisOptions {
            quality: quality.clamp(1, 100),
            ..self
        }
    }
}

/// Encodes `image` as a KTX2 file with Basis Universal supercompression and the defaults of
/// [`BasisOptions::new`].
pub fn encode_ktx2(image: &DecodedImage, mode: BasisMode) -> Result<Vec<u8>, ImageError> {
    encode_ktx2_with(image, &BasisOptions::new(mode))
}

/// Encodes `image` (any channel count and sample type: converted to RGBA 8-bit, grey
/// replicated, missing alpha opaque) as a Basis Universal KTX2 file.
pub fn encode_ktx2_with(
    image: &DecodedImage,
    options: &BasisOptions,
) -> Result<Vec<u8>, ImageError> {
    crate::formats::check(image)?;
    if image.width == 0 || image.height == 0 {
        return Err(ImageError::Encode("KTX2: empty image".into()));
    }
    if !CAN_ENCODE {
        return encoder::encode(Vec::new(), 0, 0, options);
    }
    let (w, h) = ktx2_size(image.width, image.height);
    let mut rgba = image.with_channels(4)?.to_u8();
    if (w, h) != (image.width, image.height) {
        frap_math::budget::reserve_buffer("KTX2 image", Some(u64::from(w) * u64::from(h) * 4))
            .map_err(|e| ImageError::Encode(format!("KTX2: {e}")))?;
        rgba = resample_rgba8(&rgba, image.width, image.height, w, h);
    }
    let mut ktx = encoder::encode(rgba, w, h, options)?;
    if !options.srgb {
        set_colour_primaries(&mut ktx, DF_PRIMARIES_UNSPECIFIED);
    }
    Ok(ktx)
}

/// `KHR_DF_PRIMARIES_UNSPECIFIED`.
pub const DF_PRIMARIES_UNSPECIFIED: u8 = 0;

/// The size a Basis Universal KTX2 file of a `width` x `height` image is written at: each
/// dimension rounded up to a multiple of 4 (`KHR_texture_basisu`: "pixelWidth and pixelHeight
/// MUST be multiples of 4").
pub fn ktx2_size(width: u32, height: u32) -> (u32, u32) {
    let up = |v: u32| v.max(1).div_ceil(4).saturating_mul(4);
    (up(width), up(height))
}

/// Sets the DFD `colorPrimaries` of a KTX2 file (byte 1 of the basic descriptor block's
/// third word: `dfdByteOffset` + the 4-byte `dfdTotalSize` + the 8-byte block header + 1).
/// No checksum covers the DFD, so the file stays valid.
fn set_colour_primaries(ktx: &mut [u8], primaries: u8) {
    let at = ktx
        .get(48..52)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize + 13);
    if let Some(p) = at.and_then(|at| ktx.get_mut(at)) {
        *p = primaries;
    }
}

/// Bilinear resampling of tightly packed RGBA8 rows from `w` x `h` to `nw` x `nh` (sample
/// centres aligned, edges clamped).
fn resample_rgba8(src: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    let (w, h, nw, nh) = (w as usize, h as usize, nw as usize, nh as usize);
    // Source index pairs and weights along one axis.
    let taps = |n: usize, m: usize| -> Vec<(usize, usize, f32)> {
        (0..m)
            .map(|i| {
                let x = ((i as f32 + 0.5) * n as f32 / m as f32 - 0.5).max(0.0);
                let i0 = (x.floor() as usize).min(n - 1);
                let i1 = (i0 + 1).min(n - 1);
                (i0, i1, x - i0 as f32)
            })
            .collect()
    };
    let (xs, ys) = (taps(w, nw), taps(h, nh));
    let mut out = Vec::with_capacity(nw * nh * 4);
    for &(y0, y1, fy) in &ys {
        for &(x0, x1, fx) in &xs {
            let px = |x: usize, y: usize, c: usize| f32::from(src[(y * w + x) * 4 + c]);
            for c in 0..4 {
                let top = px(x0, y0, c) * (1.0 - fx) + px(x1, y0, c) * fx;
                let bottom = px(x0, y1, c) * (1.0 - fx) + px(x1, y1, c) * fx;
                let v = top * (1.0 - fy) + bottom * fy;
                out.push(v.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}

#[cfg(not(feature = "basisu-encode"))]
mod encoder {
    use super::*;

    pub(super) fn encode(
        _: Vec<u8>,
        _: u32,
        _: u32,
        _: &BasisOptions,
    ) -> Result<Vec<u8>, ImageError> {
        Err(ImageError::Unsupported(
            "KTX2 encoding needs frap built with the `basisu-encode` feature".into(),
        ))
    }
}

#[cfg(feature = "basisu-encode")]
mod encoder {
    use basisu_c_sys::BasisTextureFormat;
    use basisu_c_sys::common;
    use basisu_c_sys::extra::{
        BasisuEncoder, BasisuEncoderParams, SourceImage, SourceImageFormat, basisu_encoder_init,
        types::Extent3d,
    };

    use super::*;

    /// The `TIMEOUT` reason of a Basis encode that never got a worker slot.
    const SLOTS_BUSY: &str = "the KTX2 (Basis Universal) encode waited for an encoder slot: \
        earlier image encodes that timed out are still running in the background (at most 2 \
        at once per process); retry once they finish";

    /// Encodes `w` x `h` RGBA8 rows (top row first).
    pub(super) fn encode(
        rgba: Vec<u8>,
        w: u32,
        h: u32,
        o: &BasisOptions,
    ) -> Result<Vec<u8>, ImageError> {
        let o = *o;
        crate::encode::cancellable_with(SLOTS_BUSY, "KTX2", move || {
            basisu_encoder_init();
            let mut enc = BasisuEncoder::new();
            enc.set_image(SourceImage {
                data: &rgba,
                format: SourceImageFormat::Rgba8,
                size: Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            })
            .map_err(|e| ImageError::Encode(format!("KTX2: {e}")))?;
            let format = match o.mode {
                BasisMode::Etc1s => BasisTextureFormat::Etc1s,
                BasisMode::Uastc => BasisTextureFormat::UastcLdr4x4,
            };
            let mut params = if o.srgb {
                BasisuEncoderParams::new_with_srgb_defaults(format)
            } else {
                BasisuEncoderParams::new_with_linear_defaults(format)
            };
            params.quality_level = i32::from(o.quality.clamp(1, 100));
            if o.mipmaps {
                params = params.with_flags(if o.wrap {
                    common::BU_COMP_FLAGS_GEN_MIPS_WRAP
                } else {
                    common::BU_COMP_FLAGS_GEN_MIPS_CLAMP
                });
            }
            enc.compress(params)
                .map_err(|e| ImageError::Encode(format!("KTX2: {e}")))
        })
    }
}

/// `(width, height, channels, bit depth)` of a Basis Universal KTX2 file, as
/// [`transcode`] gives it.
#[cfg(feature = "basisu")]
pub(crate) fn info(data: &[u8]) -> Result<(u32, u32, u8, u8), ImageError> {
    let t = transcoder(data)?;
    let (w, h) = t.base_dimensions();
    let channels = if t.has_alpha() { 4 } else { 3 };
    Ok((w, h, channels, if is_hdr(&t) { 32 } else { 8 }))
}

#[cfg(feature = "basisu")]
fn transcoder(data: &[u8]) -> Result<basisu::Transcoder<'_>, ImageError> {
    basisu::Transcoder::new(data)
        .map_err(|e| ImageError::Decode(format!("KTX2 (Basis Universal): {e:?}")))
}

#[cfg(feature = "basisu")]
fn is_hdr(t: &basisu::Transcoder) -> bool {
    use basisu::SourceFormat as S;
    matches!(
        t.source_format(),
        S::UastcHdr4x4 | S::AstcHdr6x6 | S::UastcHdr6x6
    )
}

/// Transcodes the base level (first layer and face) of a Basis Universal KTX2 file to RGB or
/// RGBA (RGBA when the texture has alpha): 8-bit for LDR codecs, float for HDR ones.
#[cfg(feature = "basisu")]
pub(crate) fn transcode(data: &[u8]) -> Result<DecodedImage, ImageError> {
    use crate::codec::PixelData;
    use basisu::{DecodeFlags, TargetFormat};
    let t = transcoder(data)?;
    let (w, h) = t.base_dimensions();
    let hdr = is_hdr(&t);
    let bpp: u64 = if hdr { 8 } else { 4 };
    frap_math::budget::reserve_buffer(
        "KTX2 image",
        u64::from(w)
            .checked_mul(u64::from(h))
            .and_then(|n| n.checked_mul(bpp * 2)),
    )
    .map_err(|e| ImageError::Decode(format!("KTX2: {e}")))?;
    let target = if hdr {
        TargetFormat::RgbaHalf
    } else {
        TargetFormat::Rgba32
    };
    let out = t
        .transcode_image(0, 0, 0, target, DecodeFlags::NONE)
        .map_err(|e| ImageError::Decode(format!("KTX2 (Basis Universal): {e:?}")))?;
    let alpha = t.has_alpha();
    let img = if hdr {
        let f: Vec<f32> = out
            .chunks_exact(2)
            .map(|c| half::f16::from_le_bytes([c[0], c[1]]).to_f32())
            .collect();
        DecodedImage::new(w, h, 4, PixelData::F32(f))?
    } else {
        DecodedImage::new(w, h, 4, PixelData::U8(out))?
    };
    if alpha { Ok(img) } else { img.with_channels(3) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::PixelData;

    /// A 64x32 RGB(A) of smooth gradients (alpha: opaque left half, 96 right).
    fn gradient(alpha: bool) -> DecodedImage {
        let (w, h) = (64u32, 32u32);
        let c = if alpha { 4 } else { 3 };
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                v.extend_from_slice(&[(x * 4) as u8, (y * 8) as u8, (128 + x - y) as u8]);
                if alpha {
                    v.push(if x < 32 { 255 } else { 96 });
                }
            }
        }
        DecodedImage::new(w, h, c, PixelData::U8(v)).unwrap()
    }

    #[test]
    fn modes_and_options() {
        assert_eq!(BasisMode::from_identifier("ETC1S"), Some(BasisMode::Etc1s));
        assert_eq!(BasisMode::from_identifier("UASTC"), Some(BasisMode::Uastc));
        assert_eq!(BasisMode::from_identifier("BC7"), None);
        assert_eq!(BasisMode::default(), BasisMode::Etc1s);
        let o = BasisOptions::new(BasisMode::Uastc)
            .with_quality(0)
            .with_srgb(false);
        assert_eq!(
            (o.quality, o.srgb, o.mipmaps, o.wrap),
            (1, false, true, true)
        );
        let e = encode_ktx2(
            &DecodedImage::new(0, 0, 4, PixelData::U8(vec![])).unwrap(),
            BasisMode::Etc1s,
        );
        assert!(e.is_err());
    }

    #[test]
    fn sizes_round_up_to_multiples_of_4() {
        assert_eq!(ktx2_size(5, 3), (8, 4));
        assert_eq!(ktx2_size(1, 1), (4, 4));
        assert_eq!(ktx2_size(64, 32), (64, 32));
        assert_eq!(ktx2_size(13, 4), (16, 4));
        // A constant image stays constant; a horizontal ramp stays monotonic and keeps its ends.
        let c = resample_rgba8(&[10, 20, 30, 40].repeat(15), 5, 3, 8, 4);
        assert_eq!(c, [10, 20, 30, 40].repeat(32));
        let ramp: Vec<u8> = (0..5u8).flat_map(|x| [x * 60, 0, 0, 255]).collect();
        let r = resample_rgba8(&ramp, 5, 1, 8, 4);
        let row: Vec<u8> = r[..32].chunks_exact(4).map(|p| p[0]).collect();
        assert!(row.windows(2).all(|w| w[0] <= w[1]), "{row:?}");
        assert_eq!((row[0], row[7]), (0, 240));
        // The DFD at 60: colorPrimaries is byte 60 + 13.
        let mut k = vec![9u8; 80];
        k[48..52].copy_from_slice(&60u32.to_le_bytes());
        set_colour_primaries(&mut k, 0);
        assert_eq!((k[72], k[73], k[74]), (9, 0, 9));
        set_colour_primaries(&mut [0u8; 8], 0); // too short: left alone
    }

    #[cfg(not(feature = "basisu-encode"))]
    #[test]
    fn without_the_feature_encoding_is_unsupported() {
        let e = encode_ktx2(&gradient(false), BasisMode::Etc1s).unwrap_err();
        assert!(e.to_string().contains("`basisu-encode` feature"), "{e}");
    }

    /// Mean absolute difference per sample.
    #[cfg(feature = "basisu-encode")]
    fn mean_diff(a: &DecodedImage, b: &DecodedImage) -> f64 {
        let (a, b) = (a.to_u8(), b.to_u8());
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(&b)
            .map(|(x, y)| f64::from(x.abs_diff(*y)))
            .sum::<f64>()
            / a.len() as f64
    }

    #[cfg(feature = "basisu-encode")]
    #[test]
    fn encode_then_transcode_round_trips() {
        for mode in [BasisMode::Etc1s, BasisMode::Uastc] {
            for alpha in [false, true] {
                let src = gradient(alpha);
                let ktx = encode_ktx2(&src, mode).unwrap();
                assert!(crate::ktx2::is_ktx2(&ktx));
                let r = ktx2::Reader::new(&ktx[..]).unwrap();
                let h = r.header();
                assert_eq!(h.format, None, "Basis files are VK_FORMAT_UNDEFINED");
                // Full mip chain: 64x32 down to 1x1.
                assert_eq!(h.level_count, 7);
                let scheme = h.supercompression_scheme.map(|s| s.value());
                match mode {
                    BasisMode::Etc1s => assert_eq!(scheme, Some(1), "BasisLZ"),
                    BasisMode::Uastc => assert_eq!(scheme, Some(2), "Zstandard"),
                }
                assert_eq!(r.transfer_function(), Some(ktx2::TransferFunction::SRGB));
                let d = crate::decode(&ktx).unwrap();
                assert_eq!((d.width, d.height), (64, 32));
                assert_eq!(d.channels, if alpha { 4 } else { 3 }, "{mode:?}");
                let tol = match mode {
                    BasisMode::Etc1s => 12.0,
                    BasisMode::Uastc => 3.0,
                };
                let m = mean_diff(&d, &src);
                assert!(
                    m < tol,
                    "{mode:?} alpha={alpha}: mean error {m}\n{:?}\n{:?}",
                    &d.to_u8()[..48],
                    &src.to_u8()[..48]
                );
                let i = crate::probe(&ktx).unwrap();
                assert_eq!(
                    (i.width, i.height, i.channels, i.bit_depth),
                    (64, 32, d.channels, 8)
                );
            }
        }
        // Linear data, no mips.
        let o = BasisOptions::new(BasisMode::Uastc)
            .with_srgb(false)
            .with_mipmaps(false);
        let ktx = encode_ktx2_with(&gradient(false), &o).unwrap();
        let r = ktx2::Reader::new(&ktx[..]).unwrap();
        assert_eq!(r.transfer_function(), Some(ktx2::TransferFunction::Linear));
        assert_eq!(r.header().level_count.max(1), 1);
        // KHR_texture_basisu: non-colour data has the primaries KHR_DF_PRIMARIES_UNSPECIFIED,
        // colour BT709 (DFD basic block word 2: colorModel, colorPrimaries, transferFunction).
        let dfd = u32::from_le_bytes(ktx[48..52].try_into().unwrap()) as usize + 12;
        assert_eq!(ktx[dfd..dfd + 3], [166, 0, 1]);
        let srgb = encode_ktx2(&gradient(false), BasisMode::Etc1s).unwrap();
        let dfd = u32::from_le_bytes(srgb[48..52].try_into().unwrap()) as usize + 12;
        assert_eq!(srgb[dfd..dfd + 3], [163, 1, 2]);
        // An odd size is written at the next multiple of 4 (pixelWidth / pixelHeight).
        for (w, h, want) in [(5u32, 3u32, (8, 4)), (1, 1, (4, 4))] {
            let px: Vec<u8> = (0..w * h * 3).map(|i| (i * 37 % 256) as u8).collect();
            let img = DecodedImage::new(w, h, 3, PixelData::U8(px)).unwrap();
            let k = encode_ktx2_with(&img, &o).unwrap();
            let hdr = ktx2::Reader::new(&k[..]).unwrap().header();
            assert_eq!((hdr.pixel_width, hdr.pixel_height), want);
            let d = crate::decode(&k).unwrap();
            assert_eq!((d.width, d.height), want);
        }
        // Grey input is replicated.
        let g = DecodedImage::new(4, 4, 1, PixelData::U8(vec![200; 16])).unwrap();
        let d = crate::decode(&encode_ktx2(&g, BasisMode::Uastc).unwrap()).unwrap();
        assert!(d.to_u8().iter().all(|v| v.abs_diff(200) <= 2));
    }
}
