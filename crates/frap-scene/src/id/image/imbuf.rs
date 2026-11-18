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
}

/// Channels of every frap image buffer (`ibuf->channels`; Blender loads every 8-bit file and
/// every generated image as RGBA).
pub const CHANNELS: usize = 4;

impl ImBuf {
    /// A buffer of `x × y` pixels, every channel zero.
    pub fn new(x: u32, y: u32, planes: u8, float: bool) -> ImBuf {
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
        let planes = if self.planes == 0 { 24 } else { self.planes };
        let bits = i32::from(planes.max(8) / 8 * 8);
        if self.is_float() { bits * 4 } else { bits }
    }

    /// `rna_Image_pixels_get`: floats as stored, bytes divided by 255.
    pub fn pixels(&self) -> Vec<f32> {
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
    fn rows_flip() {
        let d = [1, 2, 3, 4, 5, 6];
        assert_eq!(flip_rows(&d, 2), vec![5, 6, 3, 4, 1, 2]);
        assert_eq!(flip_rows(&d, 0), d.to_vec());
    }
}
