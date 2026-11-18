// SPDX-License-Identifier: GPL-3.0-or-later
//! `ImBuf`: the in-memory pixel buffer of an image (Blender's `ImBuf` with its byte or float
//! buffer), as `Image.pixels`, `Image.scale()` and the packers see it.
//!
//! Layout as Blender's: four interleaved channels per pixel, **bottom row first**
//! (`pixels[0]` is the bottom-left pixel's red). A byte buffer holds the image's own colour
//! space (sRGB bytes for a generated image); a float buffer is scene linear.
//!
//! Conversions follow `BLI_math_base_inline.cc` / `BLI_math_color.cc` (Blender 5.2.4,
//! GPL-2.0-or-later): `unit_float_to_uchar_clamp`, `srgb_to_linearrgb`, `hsv_to_rgb`.

use smol_str::SmolStr;

use crate::generated::plain::ColorManagedInputColorspaceSettings;

/// What a statement's budget calls an image buffer in `LIMIT_EXCEEDED` messages.
pub const BUFFER: &str = "image buffer";

/// Bytes of an `x × y` buffer with four channels (`None` on overflow).
pub fn buffer_bytes(x: u32, y: u32, float: bool) -> Option<u64> {
    let per_sample = if float { 4 } else { 1 };
    u64::from(x)
        .checked_mul(u64::from(y))?
        .checked_mul(CHANNELS as u64 * per_sample)
}

/// Byte or float pixels (four channels, bottom row first).
#[derive(Clone, Debug, PartialEq)]
pub enum ImBufData {
    /// `ibuf->byte_buffer` (8 bits per channel).
    Byte(Vec<u8>),
    /// `ibuf->float_buffer` (32-bit float, scene linear).
    Float(Vec<f32>),
}

/// An image buffer (Blender's `ImBuf`, one view, one tile).
#[derive(Clone, Debug, PartialEq)]
pub struct ImBuf {
    /// `ibuf->x`
    pub x: u32,
    /// `ibuf->y`
    pub y: u32,
    /// `ibuf->planes`: bits per pixel of the source colour mode (8 grey, 16 grey + alpha,
    /// 24 RGB, 32 RGBA), what `Image.depth` reports (times 4 for a float buffer).
    pub planes: u8,
    /// Pixels.
    pub data: ImBufData,
    /// `IB_BITMAPDIRTY`: changed since it was loaded, generated, packed or saved
    /// (`Image.is_dirty`).
    pub dirty: bool,
    /// `ibuf->ftype` as an `Image.file_format` identifier (`PNG`, `OPEN_EXR`, …).
    pub ftype: SmolStr,
    /// The image's colour space settings when the buffer was loaded or generated
    /// (`Image::acquire` sets them; `None`: not from an image). Blender frees an image's
    /// buffers when `colorspace_settings` change (`IMA_SIGNAL_COLORMANAGE`); frap keeps a
    /// stored buffer only while the settings still match (see `Image::live_buffer`).
    pub colorspace: Option<ColorManagedInputColorspaceSettings>,
}

/// Channels of every frap image buffer (`ibuf->channels`; Blender loads every 8-bit file and
/// every generated image as RGBA).
pub const CHANNELS: usize = 4;

impl ImBuf {
    /// A buffer of `x × y` pixels, every channel zero.
    ///
    /// The size is checked against the statement's budget first
    /// (`frap_math::budget::check_buffer`): a buffer larger than `--max-elements` allows
    /// unwinds with `LIMIT_EXCEEDED` instead of allocating.
    pub fn new(x: u32, y: u32, planes: u8, float: bool) -> ImBuf {
        frap_math::budget::check_buffer(BUFFER, buffer_bytes(x, y, float));
        let n = x as usize * y as usize * CHANNELS;
        ImBuf {
            x,
            y,
            planes,
            data: if float {
                ImBufData::Float(vec![0.0; n])
            } else {
                ImBufData::Byte(vec![0; n])
            },
            dirty: false,
            ftype: SmolStr::new_static("PNG"),
            colorspace: None,
        }
    }

    /// True for a float buffer (`Image.is_float`).
    pub fn is_float(&self) -> bool {
        matches!(self.data, ImBufData::Float(_))
    }

    /// Number of samples (`IMB_get_pixel_count(ibuf) * ibuf->channels`).
    pub fn len(&self) -> usize {
        self.x as usize * self.y as usize * CHANNELS
    }

    /// True when the buffer has no pixel.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `Image.depth`: `color_mode_channels_get() * 8`, times 4 for a float buffer.
    /// A colour mode of 0 (a FILE image switched to GENERATED keeps `gen_depth` 0) counts as
    /// RGB (verified on 5.2.4: depth 24).
    pub fn depth(&self) -> i32 {
        depth_of(self.planes, self.is_float())
    }

    /// `imb_handle_colorspace_and_alpha` + `colormanage_imbuf_make_linear` (`readimage.cc`,
    /// `colormanagement.cc`, Blender 5.2.4, GPL-2.0-or-later) for a buffer just decoded from a
    /// file (straight samples in the file's colour space): applies the image's `alpha_mode`
    /// and converts a float buffer to scene linear.
    ///
    /// - a data colour space (`is_data`) or `CHANNEL_PACKED`: alpha is not touched;
    ///   `NONE`: alpha becomes opaque; `PREMUL`: a byte buffer is unpremultiplied (floats are
    ///   expected premultiplied); `STRAIGHT`: a float buffer is premultiplied (bytes are
    ///   expected straight);
    /// - a float buffer in a non-data colour space is converted to scene linear, with
    ///   OpenColorIO's `predivide` (unpremultiply, convert, premultiply; pixels with alpha 0 or
    ///   1 are converted as they are) unless the channels are packed. Byte buffers keep the
    ///   file's colour space (Blender converts them when they are used).
    pub fn handle_colorspace_and_alpha(
        &mut self,
        colorspace: &str,
        is_data: bool,
        alpha_mode: &str,
    ) {
        let packed = is_data || alpha_mode == "CHANNEL_PACKED";
        let row = (self.x as usize * CHANNELS).max(CHANNELS);
        match (&mut self.data, alpha_mode) {
            _ if packed => {}
            (ImBufData::Byte(b), "NONE") => b.chunks_exact_mut(4).for_each(|p| p[3] = 255),
            (ImBufData::Float(f), "NONE") => f.chunks_exact_mut(4).for_each(|p| p[3] = 1.0),
            // `IMB_unpremultiply_rect`
            (ImBufData::Byte(b), "PREMUL") => {
                for r in b.chunks_mut(row) {
                    frap_math::budget::check_n(self.x);
                    for p in r.chunks_exact_mut(4) {
                        let val = if p[3] != 0 {
                            1.0 / f32::from(p[3])
                        } else {
                            1.0
                        };
                        for c in &mut p[..3] {
                            *c = unit_float_to_uchar_clamp(f32::from(*c) * val);
                        }
                    }
                }
            }
            (ImBufData::Float(_), "PREMUL") | (ImBufData::Byte(_), _) => {}
            // `IMB_premultiply_rect_float`
            (ImBufData::Float(f), _) => {
                for r in f.chunks_mut(row) {
                    frap_math::budget::check_n(self.x);
                    for p in r.chunks_exact_mut(4) {
                        let val = p[3];
                        for c in &mut p[..3] {
                            *c *= val;
                        }
                    }
                }
            }
        }
        if is_data {
            return;
        }
        let ImBufData::Float(f) = &mut self.data else {
            return;
        };
        let Some(curve) = to_linear_curve(colorspace) else {
            return;
        };
        for r in f.chunks_mut(row) {
            frap_math::budget::check_n(self.x);
            for p in r.chunks_exact_mut(4) {
                let a = p[3];
                let predivide = !packed && a != 0.0 && a != 1.0;
                if predivide {
                    let inv = 1.0 / a;
                    for c in &mut p[..3] {
                        *c *= inv;
                    }
                }
                for c in &mut p[..3] {
                    *c = curve(*c);
                }
                if predivide {
                    for c in &mut p[..3] {
                        *c *= a;
                    }
                }
            }
        }
    }

    /// `rna_Image_pixels_get`: floats as stored, bytes divided by 255. The array is checked
    /// against the statement's budget first (a byte buffer reads as four times its size).
    pub fn pixels(&self) -> Vec<f32> {
        frap_math::budget::check_buffer(BUFFER, buffer_bytes(self.x, self.y, true));
        match &self.data {
            ImBufData::Float(f) => f.clone(),
            ImBufData::Byte(b) => b.iter().map(|&v| f32::from(v) * (1.0 / 255.0)).collect(),
        }
    }

    /// `rna_Image_pixels_set`: floats stored as given, bytes through
    /// `unit_float_to_uchar_clamp`; marks the buffer dirty. `values` must have [`Self::len`]
    /// samples.
    pub fn set_pixels(&mut self, values: &[f32]) {
        match &mut self.data {
            ImBufData::Float(f) => f.copy_from_slice(values),
            ImBufData::Byte(b) => {
                for (d, &v) in b.iter_mut().zip(values) {
                    *d = unit_float_to_uchar_clamp(v);
                }
            }
        }
        self.dirty = true;
    }

    /// Reverses the row order of samples in rows of `row` samples (bottom first ↔ top first,
    /// the order of image files).
    pub fn flip_rows<T: Copy>(data: &[T], row: usize) -> Vec<T> {
        flip_rows(data, row)
    }
}

/// `Image.depth` of a colour mode (`planes`) and buffer type (see [`ImBuf::depth`]).
pub fn depth_of(planes: u8, float: bool) -> i32 {
    let planes = if planes == 0 { 24 } else { planes };
    let bits = i32::from(planes.max(8) / 8 * 8);
    if float { bits * 4 } else { bits }
}

/// The transfer function from a colour space to scene linear (`Linear Rec.709` in Blender's
/// configuration), `None` when the space is linear (no conversion).
///
/// frap converts with the sRGB curve for `sRGB` and every other display-referred space, and
/// treats every `Linear …` and ACES space as scene linear (Blender also converts their
/// primaries; frap has no gamut conversions, see docs/deviations.md).
pub fn to_linear_curve(colorspace: &str) -> Option<fn(f32) -> f32> {
    if colorspace.is_empty()
        || colorspace.starts_with("Linear")
        || colorspace.starts_with("ACES")
        || colorspace == "Non-Color"
    {
        None
    } else {
        Some(srgb_to_linearrgb)
    }
}

/// Reverses the row order of a buffer of rows of `row` samples (top first ↔ bottom first).
fn flip_rows<T: Copy>(data: &[T], row: usize) -> Vec<T> {
    if row == 0 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    for r in data.chunks_exact(row).rev() {
        out.extend_from_slice(r);
    }
    out
}

/// `unit_float_to_uchar_clamp` (NaN → 0).
#[inline]
pub fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 || v.is_nan() {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// `srgb_to_linearrgb`
#[inline]
pub fn srgb_to_linearrgb(c: f32) -> f32 {
    if c < 0.04045 {
        return if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) };
    }
    ((c + 0.055) * (1.0 / 1.055)).powf(2.4)
}

/// `hsv_to_rgb`
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let nr = ((h * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let ng = (2.0 - (h * 6.0 - 2.0).abs()).clamp(0.0, 1.0);
    let nb = (2.0 - (h * 6.0 - 4.0).abs()).clamp(0.0, 1.0);
    [
        ((nr - 1.0) * s + 1.0) * v,
        ((ng - 1.0) * s + 1.0) * v,
        ((nb - 1.0) * s + 1.0) * v,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_round_trip_bytes() {
        // Verified on 5.2.4: `pixels[0:4] = (0.5, 0.25, 1.2, -0.1)` on a byte image reads
        // back (0.501961, 0.250980, 1.0, 0.0).
        let mut b = ImBuf::new(1, 1, 32, false);
        b.set_pixels(&[0.5, 0.25, 1.2, -0.1]);
        let p = b.pixels();
        assert_eq!(p[0], 128.0 / 255.0);
        assert_eq!(p[1], 64.0 / 255.0);
        assert_eq!(p[2], 1.0);
        assert_eq!(p[3], 0.0);
        assert!(b.dirty);
    }

    #[test]
    fn depth_per_planes() {
        // Verified on 5.2.4: grey 8, grey+alpha 16, RGB 24, RGBA 32, 16-bit RGB (float) 96.
        assert_eq!(ImBuf::new(1, 1, 8, false).depth(), 8);
        assert_eq!(ImBuf::new(1, 1, 16, false).depth(), 16);
        assert_eq!(ImBuf::new(1, 1, 24, false).depth(), 24);
        assert_eq!(ImBuf::new(1, 1, 32, false).depth(), 32);
        assert_eq!(ImBuf::new(1, 1, 24, true).depth(), 96);
        assert_eq!(ImBuf::new(1, 1, 32, true).depth(), 128);
    }

    #[test]
    fn file_colorspace_and_alpha() {
        // Verified on 5.2.4: a 16-bit RGBA PNG holding (0.69837, 0.96905, 0.4956, 0.5) and
        // (1, 0.88367, 1, 0.25) reads, per colour space and alpha mode:
        let file = [0.69837, 0.96905, 0.4956, 0.5, 1.0, 0.88367, 1.0, 0.25];
        let cases: [(&str, bool, &str, [f32; 8]); 6] = [
            (
                "sRGB",
                false,
                "STRAIGHT",
                [
                    0.22284, 0.46552, 0.10499, 0.5, 0.25001, 0.18887, 0.25001, 0.25,
                ],
            ),
            ("Non-Color", true, "STRAIGHT", file),
            (
                "sRGB",
                false,
                "CHANNEL_PACKED",
                [
                    0.44567, 0.93103, 0.20999, 0.5, 1.00002, 0.75549, 1.00002, 0.25,
                ],
            ),
            (
                "sRGB",
                false,
                "PREMUL",
                [
                    1.0757, 2.30157, 0.49004, 0.5, 6.32839, 4.72348, 6.32839, 0.25,
                ],
            ),
            (
                "sRGB",
                false,
                "NONE",
                [
                    0.44567, 0.93103, 0.20999, 1.0, 1.00002, 0.75549, 1.00002, 1.0,
                ],
            ),
            ("Non-Color", true, "NONE", file),
        ];
        for (cs, data, alpha, want) in cases {
            let mut b = ImBuf::new(2, 1, 32, true);
            b.data = ImBufData::Float(file.to_vec());
            b.handle_colorspace_and_alpha(cs, data, alpha);
            for (g, w) in b.pixels().iter().zip(want) {
                assert!(
                    (g - w).abs() < 2e-4 * w.max(1.0),
                    "{cs} {alpha}: {:?}",
                    b.pixels()
                );
            }
        }
        // An OpenEXR packed from memory keeps the image's STRAIGHT: reloading premultiplies.
        let mut b = ImBuf::new(1, 1, 32, true);
        b.data = ImBufData::Float(vec![0.1, 2.5, 0.05, 0.5]);
        b.handle_colorspace_and_alpha("Linear Rec.709", false, "STRAIGHT");
        assert_eq!(b.pixels(), vec![0.05, 1.25, 0.025, 0.5]);
        // Bytes (an 8-bit RGBA PNG (51, 102, 153, 128), (255, 128, 64, 64)): PREMUL
        // unpremultiplies, NONE makes them opaque, STRAIGHT keeps them.
        let bytes = vec![51, 102, 153, 128, 255, 128, 64, 64];
        let run = |alpha: &str| {
            let mut b = ImBuf::new(2, 1, 32, false);
            b.data = ImBufData::Byte(bytes.clone());
            b.handle_colorspace_and_alpha("sRGB", false, alpha);
            b.data
        };
        assert_eq!(
            run("PREMUL"),
            ImBufData::Byte(vec![102, 203, 255, 128, 255, 255, 255, 64])
        );
        assert_eq!(
            run("NONE"),
            ImBufData::Byte(vec![51, 102, 153, 255, 255, 128, 64, 255])
        );
        assert_eq!(run("STRAIGHT"), ImBufData::Byte(bytes.clone()));
    }

    #[test]
    fn buffers_over_the_budget_unwind() {
        let _s = frap_math::budget::scope(frap_math::budget::Budget {
            timeout: None,
            max_elements: 1000,
        });
        // 32 bytes per element: 32,000 bytes, a 40 × 50 byte buffer (8,000) fits.
        let _ok = ImBuf::new(40, 50, 32, false);
        let r = std::panic::catch_unwind(|| ImBuf::new(65536, 65536, 32, true));
        let e = frap_math::budget::elements_exceeded(&*r.unwrap_err()).unwrap();
        assert_eq!(e.buffer, Some(BUFFER));
        assert_eq!(e.corners, 1 << 36);
    }

    #[test]
    fn rows_flip() {
        let d = [1, 2, 3, 4, 5, 6];
        assert_eq!(flip_rows(&d, 2), vec![5, 6, 3, 4, 1, 2]);
        assert_eq!(flip_rows(&d, 0), d.to_vec());
    }
}
