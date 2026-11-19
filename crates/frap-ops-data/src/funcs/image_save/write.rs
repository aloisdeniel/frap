// SPDX-License-Identifier: GPL-3.0-or-later
//! Image buffer I/O shared by the `Image` RNA functions of this directory (`save`,
//! `save_render`, `pack`, `unpack`, `reload`, `scale`, …): the `frap-imbuf` decoder registered
//! for frap-scene (`IMAGE_DECODERS`), encoders with Blender's format rules, `BLI_path_abs`, and
//! the RNA plumbing (the image of `this`, reports).
//!
//! Ported rules (Blender 5.2.4, GPL-2.0-or-later):
//! - decode (`imb_loadpng` & co.): 8-bit files give a byte buffer, 16-bit PNG, OpenEXR and
//!   HDR a float buffer, with straight samples in the file's colour space (`Image::acquire`
//!   then applies the image's alpha mode and colour space, as `imb_handle_colorspace_and_alpha`
//!   does); grey expands to RGB; `planes` is the file's colour mode; the header alone is read
//!   by the registered probe (`Image.size/depth/is_float`, and the budget check before
//!   decoding);
//! - `image_memorypack_imbuf` (`image.cc`): a float buffer packs as OpenEXR, a byte buffer as
//!   PNG, in the buffer's colour mode;
//! - `BKE_image_format_from_imbuf` + `IMB_colormanagement_imbuf_for_write` +
//!   `BKE_imbuf_write_as` (`image_format.cc`, `colormanagement.cc`, `writeimage.cc`): `save()`
//!   writes the image's `file_format`, a float buffer to a display format as 16-bit PNG or
//!   8-bit otherwise, encoded to the image's colour space (`sRGB` for a generated image). The
//!   premultiplied float buffer is unpremultiplied where Blender does it (see [`Unpremul`]).

use frap_imbuf::{DecodedImage, EncodeFormat, FileFormat, PixelData};
use frap_rna::{IdRef, ImageId, PyExc, Report, ReportType, RnaError, RnaErrorKind, RnaPtr};
use frap_scene::Host;
use frap_scene::id::{ImBuf, ImBufData, ImInfo, Image, PathBase};
use smol_str::SmolStr;

use super::super::_common::{internal, root_id};

/// The `frap-imbuf` decoder, registered for `Image::acquire` / `Image::load`.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_scene::id::IMAGE_DECODERS)]
static DECODER: frap_scene::id::ImageDecoder = decode;

/// The `frap-imbuf` header reader, registered for `Image::info` / `Image::load`.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_scene::id::IMAGE_PROBES)]
static PROBE: frap_scene::id::ImageProbe = probe;

/// Encoded bytes → buffer description, from the header (what [`decode`] gives).
pub(crate) fn probe(bytes: &[u8]) -> Result<ImInfo, String> {
    let i = frap_imbuf::probe(bytes).map_err(|e| e.to_string())?;
    Ok(ImInfo {
        x: i.width,
        y: i.height,
        planes: i.channels * 8,
        float: i.bit_depth > 8,
        ftype: SmolStr::new_static(i.format.identifier()),
    })
}

/// `linearrgb_to_srgb`
fn linear_to_srgb(c: f32) -> f32 {
    if c < 0.003_130_8 {
        return if c < 0.0 { 0.0 } else { c * 12.92 };
    }
    1.055 * c.powf(1.0 / 2.4) - 0.055
}

/// Encoded bytes → buffer (see the module documentation).
pub(crate) fn decode(bytes: &[u8]) -> Result<ImBuf, String> {
    let fmt = FileFormat::detect(bytes).ok_or("unknown file-format")?;
    let d = frap_imbuf::decode(bytes).map_err(|e| e.to_string())?;
    let c = usize::from(d.channels);
    let row = d.width as usize * 4;
    let rgba = |s: &[f32], one: f32| -> [f32; 4] {
        match c {
            1 => [s[0], s[0], s[0], one],
            2 => [s[0], s[0], s[0], s[1]],
            3 => [s[0], s[1], s[2], one],
            _ => [s[0], s[1], s[2], s[3]],
        }
    };
    let data = match &d.pixels {
        PixelData::U8(v) => {
            let mut out = Vec::with_capacity(v.len() / c * 4);
            for s in v.chunks_exact(c) {
                let f: Vec<f32> = s.iter().map(|&b| f32::from(b)).collect();
                out.extend(rgba(&f, 255.0).map(|x| x as u8));
            }
            ImBufData::Byte(ImBuf::flip_rows(&out, row))
        }
        PixelData::U16(v) => {
            // 16-bit PNG: float buffer of the file's (straight) samples.
            let mut out = Vec::with_capacity(v.len() / c * 4);
            for s in v.chunks_exact(c) {
                let f: Vec<f32> = s.iter().map(|&x| f32::from(x) / 65535.0).collect();
                out.extend(rgba(&f, 1.0));
            }
            ImBufData::Float(ImBuf::flip_rows(&out, row))
        }
        PixelData::F32(v) => {
            let mut out = Vec::with_capacity(v.len() / c * 4);
            for s in v.chunks_exact(c) {
                out.extend(rgba(s, 1.0));
            }
            ImBufData::Float(ImBuf::flip_rows(&out, row))
        }
        _ => return Err("unsupported sample type".into()),
    };
    Ok(ImBuf {
        x: d.width,
        y: d.height,
        planes: (c * 8) as u8,
        data,
        dirty: false,
        ftype: SmolStr::new_static(fmt.identifier()),
        colorspace: None,
    })
}

/// Output channels of a buffer's colour mode (`planes`): 1 BW, 3 RGB, 4 RGBA (grey + alpha
/// writes as RGBA, Blender has no BW + alpha mode).
pub(crate) fn mode_channels(planes: u8) -> u8 {
    match planes {
        8 => 1,
        24 | 0 => 3,
        _ => 4,
    }
}

/// `BKE_imbuf_alpha_test`: some pixel is not opaque.
pub(crate) fn alpha_test(buf: &ImBuf) -> bool {
    match &buf.data {
        ImBufData::Byte(b) => b.chunks_exact(4).any(|p| p[3] != 255),
        ImBufData::Float(f) => f.chunks_exact(4).any(|p| p[3] != 1.0),
    }
}

/// `IMB_alpha_under_color_byte` with a black background (one RGBA pixel): the colour times
/// alpha, truncated, alpha 255.
pub(crate) fn alpha_under_black_byte(p: &mut [u8]) {
    match p[3] {
        255 => {}
        0 => p[..3].fill(0),
        a => {
            let alpha = (f64::from(a) / 255.0) as f32;
            for c in &mut p[..3] {
                *c = (f32::from(*c) * alpha) as u8;
            }
        }
    }
    p[3] = 255;
}

/// How the colours of a float buffer are converted on write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// Stored values (byte buffers to byte files, linear floats to float files).
    AsIs,
    /// Scene linear → sRGB (`IMB_colormanagement_transform_float` /
    /// `IMB_byte_from_float` to the image's colour space).
    LinearToSrgb,
}

/// When the premultiplied samples of a float buffer are divided by alpha on write (pixels with
/// alpha 0 or 1 are left as they are).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unpremul {
    /// Never: float files (OpenEXR, HDR), and a 16-bit PNG written without a colour space
    /// conversion from a non-data colour space (`prepare_save_png` writes the samples as
    /// unassociated alpha).
    Never,
    /// Before the colour conversion: 8-bit output (`IMB_byte_from_float` with `predivide`).
    Before,
    /// After the colour conversion: a 16-bit PNG that was converted to the image's colour
    /// space, or whose colour space is data (`prepare_save_png` sets
    /// `oiio:UnassociatedAlpha` 0 and OpenImageIO unpremultiplies).
    After,
}

/// The buffer as a top-row-first image with `channels` channels. Alpha is dropped as stored
/// unless `under_black` composites the colour over black first (`IMB_alpha_under_color_*`);
/// float colours are converted with `enc` and unpremultiplied as `unpremul` says.
///
/// Verified on 5.2.4: a float RGBA image with pixels (0.1, 0.2, 0.05, 0.5),
/// (0.5, 0.04, 0.9, 0.25) saved as 16-bit PNG from a generated image holds
/// `srgb(c) / a`: (0.69837, 0.96905, 0.4956, 0.5), (1, 0.88367, 1, 0.25); as OpenEXR
/// `srgb(c)`: (0.34919, 0.48453, 0.2478, 0.5), (0.73536, 0.22092, 0.95469, 0.25).
pub(crate) fn to_decoded(
    buf: &ImBuf,
    channels: u8,
    enc: Encoding,
    unpremul: Unpremul,
    under_black: bool,
) -> DecodedImage {
    let row = buf.x as usize * 4;
    let divide = |p: &mut [f32]| {
        let a = p[3];
        if a != 0.0 && a != 1.0 {
            let inv = 1.0 / a;
            for c in &mut p[..3] {
                *c *= inv;
            }
        }
    };
    let pixels = match &buf.data {
        ImBufData::Byte(b) => {
            let mut v = ImBuf::flip_rows(b, row);
            if under_black {
                for p in v.chunks_exact_mut(4) {
                    alpha_under_black_byte(p);
                }
            }
            PixelData::U8(v)
        }
        ImBufData::Float(f) => {
            let mut v = ImBuf::flip_rows(f, row);
            for p in v.chunks_exact_mut(4) {
                frap_math::budget::check();
                // `IMB_alpha_under_color_float` over black: the premultiplied colour, opaque.
                if under_black {
                    p[3] = 1.0;
                }
                if unpremul == Unpremul::Before {
                    divide(p);
                }
                if enc == Encoding::LinearToSrgb {
                    for c in &mut p[..3] {
                        *c = linear_to_srgb(*c);
                    }
                }
                if unpremul == Unpremul::After {
                    divide(p);
                }
            }
            PixelData::F32(v)
        }
    };
    let img = DecodedImage::new(buf.x, buf.y, 4, pixels).expect("invariant: 4 channels");
    img.with_channels(channels)
        .expect("invariant: 1..=4 channels")
}

/// `image_memorypack_imbuf`: (bytes, MIME type, `file_format`).
pub(crate) fn memorypack(buf: &ImBuf) -> Result<(Vec<u8>, &'static str, &'static str), String> {
    let channels = mode_channels(buf.planes);
    if buf.is_float() {
        let img = to_decoded(buf, channels, Encoding::AsIs, Unpremul::Never, false);
        let bytes = frap_imbuf::encode(&img, &EncodeFormat::OpenExr { bit_depth: 32 })
            .map_err(|e| e.to_string())?;
        Ok((bytes, "image/x-exr", "OPEN_EXR"))
    } else {
        let img = to_decoded(buf, channels, Encoding::AsIs, Unpremul::Never, false);
        let bytes = frap_imbuf::encode(
            &img,
            &EncodeFormat::Png {
                bit_depth: 8,
                compression: 15,
            },
        )
        .map_err(|e| e.to_string())?;
        Ok((bytes, "image/png", "PNG"))
    }
}

/// MIME type of encoded bytes (for `EncodedImage.mime`).
pub(crate) fn mime_of(bytes: &[u8]) -> &'static str {
    match FileFormat::detect(bytes) {
        Some(FileFormat::Png) => "image/png",
        Some(FileFormat::Jpeg) => "image/jpeg",
        Some(FileFormat::WebP) => "image/webp",
        Some(FileFormat::OpenExr) => "image/x-exr",
        Some(FileFormat::Hdr) => "image/vnd.radiance",
        Some(FileFormat::Bmp) => "image/bmp",
        Some(FileFormat::Targa) => "image/x-tga",
        None => "application/octet-stream",
    }
}

/// `BKE_imbuf_write_as` file creation: `BLI_file_ensure_parent_dir_exists` then the write.
/// `path` is the path as Blender sees it (after `BLI_path_abs`, see [`PathBase::abs`]); a
/// path without a folder fails like Blender's (it cannot create the folder `''`). Relative
/// paths are written under the session's working directory.
pub(crate) fn write_file(base: &PathBase, path: &str, bytes: &[u8]) -> Result<(), String> {
    let no_folder = std::path::Path::new(path)
        .parent()
        .is_none_or(|d| d.as_os_str().is_empty());
    if path.is_empty() || no_folder {
        return Err("No such file or directory".into());
    }
    let p = base.file(path);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| frap_scene::id::io_message(&e))?;
    }
    std::fs::write(&p, bytes).map_err(|e| frap_scene::id::io_message(&e))
}

/// The image `this` points at.
pub(crate) fn image_ref(this: &RnaPtr) -> Result<IdRef<ImageId>, RnaError> {
    IdRef::from_any(root_id(this)?).ok_or_else(|| internal("the pointer is not rooted at an Image"))
}

/// The image `this` points at (read).
pub(crate) fn image<'h>(h: &'h dyn Host, this: &RnaPtr) -> Result<&'h Image, RnaError> {
    let r = image_ref(this)?;
    h.main()
        .images
        .get(r)
        .ok_or_else(|| internal("the image was removed"))
}

/// Errors and other reports of one RNA function call (`ReportList`): errors become one
/// `RuntimeError` (`"Error: a\nError: b\n"`), the rest go to the response.
#[derive(Default)]
pub(crate) struct Reports {
    errors: Vec<String>,
    other: Vec<Report>,
}

impl Reports {
    /// `BKE_report(RPT_ERROR, msg)`
    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
    /// `BKE_report(RPT_INFO, msg)`
    pub(crate) fn info(&mut self, msg: impl Into<String>) {
        self.other.push(Report::new(ReportType::Info, msg));
    }
    /// `BKE_report(RPT_WARNING, msg)`
    pub(crate) fn warning(&mut self, msg: impl Into<String>) {
        self.other.push(Report::new(ReportType::Warning, msg));
    }
    /// Sends the non-error reports and turns errors into the call's `RuntimeError`.
    pub(crate) fn finish(self, h: &mut dyn Host) -> Result<(), RnaError> {
        if self.errors.is_empty() {
            for r in self.other {
                h.report(r);
            }
            return Ok(());
        }
        let msg: String = self
            .errors
            .iter()
            .map(|e| format!("Error: {e}\n"))
            .collect();
        Err(RnaError::new(RnaErrorKind::Other, PyExc::RuntimeError, msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, channels: u8, data: Vec<u8>) -> Vec<u8> {
        let img = DecodedImage::new(w, h, channels, PixelData::U8(data)).unwrap();
        frap_imbuf::encode(
            &img,
            &EncodeFormat::Png {
                bit_depth: 8,
                compression: 15,
            },
        )
        .unwrap()
    }

    #[test]
    fn decode_png_bottom_row_first() {
        // Verified on 5.2.4: a 2 × 2 RGB PNG with rows (red, green) over (blue, white) reads
        // pixels blue, white, red, green (bottom row first), alpha 255, depth 24.
        let b = decode(&png(
            2,
            2,
            3,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        ))
        .unwrap();
        assert_eq!(b.planes, 24);
        assert_eq!(b.depth(), 24);
        assert_eq!(
            b.data,
            ImBufData::Byte(vec![
                0, 0, 255, 255, 255, 255, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255
            ])
        );
        assert_eq!(b.ftype, "PNG");
    }

    #[test]
    fn decode_grey_expands() {
        // Verified on 5.2.4: grey PNG (10, 200) → (0.039216 ×3, 1), depth 8; grey + alpha
        // (10, 100) → (0.039216 ×3, 0.392157), depth 16.
        let b = decode(&png(2, 1, 1, vec![10, 200])).unwrap();
        assert_eq!(b.depth(), 8);
        assert_eq!(
            b.data,
            ImBufData::Byte(vec![10, 10, 10, 255, 200, 200, 200, 255])
        );
        let b = decode(&png(2, 1, 2, vec![10, 100, 200, 255])).unwrap();
        assert_eq!(b.depth(), 16);
        assert_eq!(
            b.data,
            ImBufData::Byte(vec![10, 10, 10, 100, 200, 200, 200, 255])
        );
    }

    #[test]
    fn decode_png16_linear_premultiplied() {
        // Verified on 5.2.4: a 16-bit RGBA PNG (65535, 32768, 0, 32768) reads
        // (0.50002, 0.107024, 0.0, 0.500008) (OCIO's sRGB curve; within 1e-4), depth 128, in
        // an sRGB image with alpha mode STRAIGHT; the probe reads the same description.
        let img = DecodedImage::new(1, 1, 4, PixelData::U16(vec![65535, 32768, 0, 32768])).unwrap();
        let bytes = frap_imbuf::encode(
            &img,
            &EncodeFormat::Png {
                bit_depth: 16,
                compression: 15,
            },
        )
        .unwrap();
        let mut b = decode(&bytes).unwrap();
        assert_eq!(probe(&bytes).unwrap(), ImInfo::of(&b));
        assert_eq!(
            b.pixels(),
            vec![1.0, 32768.0 / 65535.0, 0.0, 32768.0 / 65535.0]
        );
        b.handle_colorspace_and_alpha("sRGB", false, "STRAIGHT");
        assert_eq!(b.depth(), 128);
        let p = b.pixels();
        for (g, w) in p.iter().zip([0.50002, 0.107024, 0.0, 0.500008]) {
            assert!((g - w).abs() < 1e-4, "{p:?}");
        }
    }

    #[test]
    fn float_alpha_writes() {
        // Verified on 5.2.4 (see `to_decoded`): float RGBA pixels saved to a 16-bit PNG
        // (Blender's file holds 45768, 63507, 32479, 32768, 65535, 57911, 65535, 16384) and to
        // OpenEXR.
        let mut b = ImBuf::new(2, 1, 32, true);
        b.data = ImBufData::Float(vec![0.1, 0.2, 0.05, 0.5, 0.5, 0.04, 0.9, 0.25]);
        let png = to_decoded(&b, 4, Encoding::LinearToSrgb, Unpremul::After, false);
        let PixelData::F32(v) = &png.pixels else {
            panic!()
        };
        let want = [0.69837, 0.96905, 0.4956, 0.5, 1.0, 0.88367, 1.0, 0.25];
        for (g, w) in v.iter().zip(want) {
            assert!((g.min(1.0) - w).abs() < 1e-4, "{v:?}");
        }
        let bytes = frap_imbuf::encode(
            &png,
            &EncodeFormat::Png {
                bit_depth: 16,
                compression: 15,
            },
        )
        .unwrap();
        let PixelData::U16(u) = frap_imbuf::decode(&bytes).unwrap().pixels else {
            panic!()
        };
        let blender = [45768u16, 63507, 32479, 32768, 65535, 57911, 65535, 16384];
        for (g, w) in u.iter().zip(blender) {
            assert!((i32::from(*g) - i32::from(w)).abs() <= 8, "{u:?}");
        }
        let exr = to_decoded(&b, 4, Encoding::LinearToSrgb, Unpremul::Never, false);
        let PixelData::F32(v) = &exr.pixels else {
            panic!()
        };
        let want = [
            0.34919, 0.48453, 0.2478, 0.5, 0.73536, 0.22092, 0.95469, 0.25,
        ];
        for (g, w) in v.iter().zip(want) {
            assert!((g - w).abs() < 1e-4, "{v:?}");
        }
        // 8-bit output unpremultiplies first: srgb(c / a).
        let jpg = to_decoded(&b, 4, Encoding::LinearToSrgb, Unpremul::Before, false);
        let PixelData::F32(v) = &jpg.pixels else {
            panic!()
        };
        assert!((v[0] - linear_to_srgb(0.2)).abs() < 1e-6);
    }

    #[test]
    fn memorypack_keeps_the_colour_mode() {
        let mut b = ImBuf::new(4, 2, 24, false);
        b.set_pixels(&[0.5; 32]);
        let (bytes, mime, ff) = memorypack(&b).unwrap();
        assert_eq!((mime, ff), ("image/png", "PNG"));
        let d = frap_imbuf::decode(&bytes).unwrap();
        assert_eq!(d.channels, 3, "24 planes pack as RGB");
        let f = ImBuf::new(2, 2, 32, true);
        let (bytes, _, ff) = memorypack(&f).unwrap();
        assert_eq!(ff, "OPEN_EXR");
        assert_eq!(frap_imbuf::decode(&bytes).unwrap().channels, 4);
    }

    #[test]
    fn write_needs_a_folder() {
        assert_eq!(
            write_file(&PathBase::default(), "gen_out.png", b"x").unwrap_err(),
            "No such file or directory"
        );
        assert!(write_file(&PathBase::default(), "", b"x").is_err());
    }
}
