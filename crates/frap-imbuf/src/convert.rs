// SPDX-License-Identifier: GPL-3.0-or-later
//! Pixel-type and channel conversions with Blender's rounding rules
//! (`BLI_math_base_inline.cc`: `unit_float_to_uchar_clamp`, `unit_float_to_ushort_clamp`;
//! GPL-2.0-or-later). Values are converted as stored: no colour-space
//! or alpha (premultiplication) change happens here.

use crate::codec::{DecodedImage, ImageError, PixelData};

/// Rec.709 luma (Blender's `IMB_colormanagement_get_luminance` coefficients for the factory
/// config), used when an image is reduced to one channel (`color_mode='BW'`).
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// `unit_float_to_uchar_clamp`; NaN → 0.
#[inline]
pub fn f32_to_u8(v: f32) -> u8 {
    if v <= 0.0 || v.is_nan() {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// `unit_float_to_ushort_clamp`; NaN → 0.
#[inline]
pub fn f32_to_u16(v: f32) -> u16 {
    if v >= 1.0 - 0.5 / 65535.0 {
        65535
    } else if v <= 0.0 || v.is_nan() {
        0
    } else {
        (v * 65535.0 + 0.5) as u16
    }
}

/// Correctly rounded narrowing (`round(v * 255 / 65535)`), the inverse of [`u8_to_u16`].
///
/// Not Blender's `unit_ushort_to_uchar` (`(v + 128) >> 8`), which maps `254 * 257` to 255: Blender
/// loads 16-bit files as float buffers and narrows those with `unit_float_to_uchar_clamp`, which
/// this matches.
#[inline]
pub fn u16_to_u8(v: u16) -> u8 {
    ((u32::from(v) * 255 + 32767) / 65535) as u8
}

/// Exact widening (`v * 257`).
#[inline]
pub fn u8_to_u16(v: u8) -> u16 {
    u16::from(v) * 257
}

impl DecodedImage {
    /// Creates an image, checking that the buffer holds `width * height * channels` samples.
    pub fn new(
        width: u32,
        height: u32,
        channels: u8,
        pixels: PixelData,
    ) -> Result<DecodedImage, ImageError> {
        if !(1..=4).contains(&channels) {
            return Err(ImageError::Unsupported(format!("{channels} channels")));
        }
        let want = width as usize * height as usize * channels as usize;
        let got = pixels.len();
        if got != want {
            return Err(ImageError::Encode(format!(
                "{width}x{height}x{channels} image needs {want} samples, buffer has {got}"
            )));
        }
        Ok(DecodedImage {
            width,
            height,
            channels,
            pixels,
        })
    }

    /// Number of pixels.
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Bits per sample of the storage (8, 16 or 32).
    pub fn bit_depth(&self) -> u8 {
        match &self.pixels {
            PixelData::U8(_) => 8,
            PixelData::U16(_) => 16,
            PixelData::F32(_) => 32,
        }
    }

    /// Samples as normalised floats (8/16-bit divided by 255/65535).
    pub fn to_f32(&self) -> Vec<f32> {
        match &self.pixels {
            PixelData::U8(v) => v.iter().map(|&x| f32::from(x) / 255.0).collect(),
            PixelData::U16(v) => v.iter().map(|&x| f32::from(x) / 65535.0).collect(),
            PixelData::F32(v) => v.clone(),
        }
    }

    /// Samples as 8-bit with Blender's rounding and clamping.
    pub fn to_u8(&self) -> Vec<u8> {
        match &self.pixels {
            PixelData::U8(v) => v.clone(),
            PixelData::U16(v) => v.iter().map(|&x| u16_to_u8(x)).collect(),
            PixelData::F32(v) => v.iter().map(|&x| f32_to_u8(x)).collect(),
        }
    }

    /// Samples as 16-bit with Blender's rounding and clamping.
    pub fn to_u16(&self) -> Vec<u16> {
        match &self.pixels {
            PixelData::U8(v) => v.iter().map(|&x| u8_to_u16(x)).collect(),
            PixelData::U16(v) => v.clone(),
            PixelData::F32(v) => v.iter().map(|&x| f32_to_u16(x)).collect(),
        }
    }

    /// Converts to `channels` (1 = BW, 2 = BW + alpha, 3 = RGB, 4 = RGBA), keeping the storage
    /// type. Grey expands to RGB, missing alpha is opaque (1.0 / 255 / 65535), alpha is dropped
    /// as stored (Blender composites over black *before* encoding, in its colour management),
    /// and RGB reduces to Rec.709 luma.
    pub fn with_channels(&self, channels: u8) -> Result<DecodedImage, ImageError> {
        if !(1..=4).contains(&channels) {
            return Err(ImageError::Unsupported(format!("{channels} channels")));
        }
        if channels == self.channels {
            return Ok(self.clone());
        }
        let pixels = match &self.pixels {
            PixelData::U8(v) => PixelData::U8(remap(v, self.channels, channels, 255u8, |c| {
                f32_to_u8(luma(c.map(|x| f32::from(x) / 255.0)))
            })),
            PixelData::U16(v) => PixelData::U16(remap(v, self.channels, channels, 65535u16, |c| {
                f32_to_u16(luma(c.map(|x| f32::from(x) / 65535.0)))
            })),
            PixelData::F32(v) => PixelData::F32(remap(v, self.channels, channels, 1.0f32, luma)),
        };
        DecodedImage::new(self.width, self.height, channels, pixels)
    }
}

impl PixelData {
    /// Number of samples.
    pub fn len(&self) -> usize {
        match self {
            PixelData::U8(v) => v.len(),
            PixelData::U16(v) => v.len(),
            PixelData::F32(v) => v.len(),
        }
    }

    /// Whether there are no samples.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[inline]
fn luma(c: [f32; 3]) -> f32 {
    LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2]
}

fn remap<T: Copy>(src: &[T], from: u8, to: u8, opaque: T, grey: impl Fn([T; 3]) -> T) -> Vec<T> {
    let from = from as usize;
    let n = src.len() / from;
    let mut out = Vec::with_capacity(n * to as usize);
    for p in src.chunks_exact(from) {
        let (rgb, alpha) = match from {
            1 => ([p[0]; 3], None),
            2 => ([p[0]; 3], Some(p[1])),
            3 => ([p[0], p[1], p[2]], None),
            _ => ([p[0], p[1], p[2]], Some(p[3])),
        };
        let is_grey = from <= 2;
        match to {
            1 => out.push(if is_grey { rgb[0] } else { grey(rgb) }),
            2 => {
                out.push(if is_grey { rgb[0] } else { grey(rgb) });
                out.push(alpha.unwrap_or(opaque));
            }
            3 => out.extend_from_slice(&rgb),
            _ => {
                out.extend_from_slice(&rgb);
                out.push(alpha.unwrap_or(opaque));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blender_rounding() {
        assert_eq!(f32_to_u8(0.5), 128);
        assert_eq!(f32_to_u8(-1.0), 0);
        assert_eq!(f32_to_u8(f32::NAN), 0);
        assert_eq!(f32_to_u8(2.0), 255);
        assert_eq!(f32_to_u16(0.5), 32768);
        assert_eq!(f32_to_u16(1.0), 65535);
        assert_eq!(u16_to_u8(65535 - 128), 255);
        assert_eq!(u16_to_u8(32767), 127);
        assert_eq!(u16_to_u8(32768), 128);
        for v in 0..=255u8 {
            assert_eq!(u16_to_u8(u8_to_u16(v)), v);
            assert_eq!(f32_to_u8(f32::from(v) / 255.0), v);
        }
        // Agrees with the float path for every 16-bit value.
        for v in 0..=65535u16 {
            assert_eq!(u16_to_u8(v), f32_to_u8(f32::from(v) / 65535.0), "{v}");
        }
    }

    #[test]
    fn channel_conversions() {
        let img = DecodedImage::new(2, 1, 3, PixelData::U8(vec![255, 0, 0, 10, 20, 30])).unwrap();
        let rgba = img.with_channels(4).unwrap();
        assert_eq!(
            rgba.pixels,
            PixelData::U8(vec![255, 0, 0, 255, 10, 20, 30, 255])
        );
        assert_eq!(rgba.with_channels(3).unwrap(), img);
        let bw = img.with_channels(1).unwrap();
        assert_eq!(bw.pixels, PixelData::U8(vec![54, 19]));
        let ga = bw.with_channels(2).unwrap();
        assert_eq!(ga.pixels, PixelData::U8(vec![54, 255, 19, 255]));
        assert_eq!(
            ga.with_channels(4).unwrap().pixels,
            PixelData::U8(vec![54, 54, 54, 255, 19, 19, 19, 255])
        );
        let f = DecodedImage::new(1, 1, 4, PixelData::F32(vec![0.5, 0.25, 1.0, 0.5])).unwrap();
        assert_eq!(f.with_channels(2).unwrap().to_f32()[1], 0.5);
        assert!(DecodedImage::new(1, 1, 3, PixelData::U8(vec![0; 4])).is_err());
        assert!(img.with_channels(5).is_err());
    }
}
