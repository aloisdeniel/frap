// SPDX-License-Identifier: GPL-3.0-or-later
//! The environment image as the world shader sees it: Blender's equirectangular mapping
//! (blender/source/blender/gpu/shaders/material/gpu_shader_material_tex_environment.glsl,
//! `node_tex_environment_equirectangular`) after the Mapping node's Z rotation, and the image
//! decoded to scene-linear RGB (the bake and the background use it at strength 1).
//!
//! Colour: float images (EXR, HDR) are scene-linear as stored; 8- and 16-bit images are
//! decoded from sRGB, Blender's default colour space for them (frap's `DecodedImage` carries
//! no colour space, so a non-colour 8-bit environment is not distinguished).

use frap_imbuf::{DecodedImage, PixelData};

/// `node_tex_environment_equirectangular` after `Rz(rotation)`: Blender's image coordinates
/// (`v = 0` is the bottom row) of world direction `d`.
pub fn uv(d: [f32; 3], rotation: f32) -> [f32; 2] {
    let (s, c) = rotation.sin_cos();
    let r = [c * d[0] - s * d[1], s * d[0] + c * d[1], d[2]];
    let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    if l == 0.0 {
        return [0.0, 0.0];
    }
    let n = r.map(|x| x / l);
    if n[0] == 0.0 || n[1] == 0.0 {
        return [0.0, 0.0];
    }
    [
        -n[1].atan2(n[0]) / std::f32::consts::TAU + 0.5,
        n[2].atan2(n[0].hypot(n[1])) / std::f32::consts::PI + 0.5,
    ]
}

/// The largest value a half-float texel holds.
pub const HALF_MAX: f32 = 65504.0;

/// A scene-linear RGB image, top row first (as decoded).
#[derive(Clone, Debug, PartialEq)]
pub struct LinearImage {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// RGB texels.
    pub rgb: Vec<[f32; 3]>,
}

impl LinearImage {
    /// `image × strength`, scene-linear (see the module documentation), clamped to the
    /// half-float range of the GPU texture.
    pub fn from_decoded(image: &DecodedImage, strength: f32) -> LinearImage {
        let ch = image.channels.max(1) as usize;
        let linear = !matches!(image.pixels, PixelData::F32(_));
        let samples = image.to_f32();
        let decode = |v: f32| {
            let l = if linear {
                frap_color::spaces::srgb_to_linear(v)
            } else {
                v
            };
            let x = l * strength;
            if x.is_nan() {
                0.0
            } else {
                x.clamp(-HALF_MAX, HALF_MAX)
            }
        };
        // A short pixel buffer yields an invalid image (`is_valid`), not a panic.
        let n = (image.width as usize * image.height as usize).min(samples.len() / ch);
        let rgb = (0..n)
            .map(|i| {
                let px = &samples[i * ch..(i + 1) * ch];
                match ch {
                    1 | 2 => [decode(px[0]); 3],
                    _ => [decode(px[0]), decode(px[1]), decode(px[2])],
                }
            })
            .collect();
        LinearImage {
            width: image.width,
            height: image.height,
            rgb,
        }
    }

    /// Blender's missing-texture colour (`node_tex_environment_empty`: magenta), 1 × 1.
    pub fn missing() -> LinearImage {
        LinearImage {
            width: 1,
            height: 1,
            rgb: vec![[1.0, 0.0, 1.0]],
        }
    }

    /// Whether the image has texels, as many as its size says.
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.rgb.len() as u64 == u64::from(self.width) * u64::from(self.height)
    }

    /// Halves the resolution (2 × 2 box) until both sides fit in `max_side`.
    pub fn fit(mut self, max_side: u32) -> LinearImage {
        while self.width > max_side || self.height > max_side {
            let (w, h) = (self.width.div_ceil(2), self.height.div_ceil(2));
            let mut rgb = Vec::with_capacity(w as usize * h as usize);
            for y in 0..h {
                for x in 0..w {
                    let mut acc = [0.0f32; 3];
                    let mut n = 0.0;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let (sx, sy) = (2 * x + dx, 2 * y + dy);
                        if sx < self.width && sy < self.height {
                            let p = self.rgb[(sy * self.width + sx) as usize];
                            for i in 0..3 {
                                acc[i] += p[i];
                            }
                            n += 1.0;
                        }
                    }
                    rgb.push(acc.map(|a| a / n));
                }
            }
            self = LinearImage {
                width: w,
                height: h,
                rgb,
            };
        }
        self
    }

    /// Bilinear sample at Blender image coordinates (`u` repeats, `v` clamps), as the GPU
    /// sampler of the world texture does.
    pub fn sample(&self, uv: [f32; 2]) -> [f32; 3] {
        let (w, h) = (self.width as i64, self.height as i64);
        let fx = uv[0] * w as f32 - 0.5;
        // Top row first: Blender's v = 1 is row 0.
        let fy = (1.0 - uv[1]) * h as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let at = |x: i64, y: i64| {
            let xx = x.rem_euclid(w);
            let yy = y.clamp(0, h - 1);
            self.rgb[(yy * w + xx) as usize]
        };
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (
            at(x0, y0),
            at(x0 + 1, y0),
            at(x0, y0 + 1),
            at(x0 + 1, y0 + 1),
        );
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bot = c[i] + (d[i] - c[i]) * tx;
            top + (bot - top) * ty
        })
    }

    /// The texels as half-float RGBA (`Rgba16Float` upload).
    pub fn to_rgba16f(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.rgb.len() * 8);
        for p in &self.rgb {
            for c in [p[0], p[1], p[2], 1.0] {
                out.extend_from_slice(&half::f16::from_f32(c).to_le_bytes());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blender_equirectangular_mapping() {
        // Values of `node_tex_environment_equirectangular`: +X looks at the image centre
        // (u = 0.5), -Y at u = 0.75, +Y at u = 0.25, the zenith at the top row (v = 1).
        let close =
            |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6;
        // Axis directions hit the `x == 0 || y == 0` guard (uv = 0).
        assert_eq!(uv([1.0, 0.0, 0.0], 0.0), [0.0, 0.0]);
        assert_eq!(uv([0.0, 0.0, 1.0], 0.0), [0.0, 0.0]);
        // Just off the axes.
        assert!(close(uv([1.0, 1e-6, 0.0], 0.0), [0.5, 0.5]));
        assert!(close(uv([1e-6, -1.0, 0.0], 0.0), [0.75, 0.5]));
        assert!(close(uv([1e-6, 1.0, 0.0], 0.0), [0.25, 0.5]));
        let up = uv([1e-4, 1e-4, 1.0], 0.0);
        assert!((up[1] - 1.0).abs() < 1e-3, "{up:?}");
        // The Mapping rotation turns the lookup: +X rotated by +90° looks up +Y.
        let r = uv([1.0, 1e-6, 0.0], std::f32::consts::FRAC_PI_2);
        assert!(close(r, uv([-1e-6, 1.0, 0.0], 0.0)), "{r:?}");
    }

    #[test]
    fn decode_and_sample() {
        let img = DecodedImage {
            width: 2,
            height: 1,
            channels: 3,
            pixels: PixelData::F32(vec![1.0, 2.0, 3.0, 1e6, f32::NAN, 0.5]),
        };
        let l = LinearImage::from_decoded(&img, 2.0);
        assert_eq!(l.rgb, vec![[2.0, 4.0, 6.0], [HALF_MAX, 0.0, 1.0]]);
        // u repeats: halfway between the last and the first texel.
        let s = l.sample([0.0, 0.5]);
        assert!((s[0] - (2.0 + HALF_MAX) / 2.0).abs() < 1.0);
        let srgb = DecodedImage {
            width: 1,
            height: 1,
            channels: 1,
            pixels: PixelData::U8(vec![255]),
        };
        assert_eq!(LinearImage::from_decoded(&srgb, 1.0).rgb, vec![[1.0; 3]]);
        let big = LinearImage {
            width: 5,
            height: 3,
            rgb: vec![[1.0; 3]; 15],
        }
        .fit(2);
        assert_eq!((big.width, big.height), (2, 1));
        assert_eq!(big.rgb, vec![[1.0; 3]; 2]);
        assert_eq!(l.to_rgba16f().len(), 16);
        assert!(l.is_valid() && LinearImage::missing().is_valid());
        let empty = LinearImage {
            width: 0,
            height: 0,
            rgb: vec![],
        };
        assert!(!empty.is_valid());
        let empty = DecodedImage {
            width: 0,
            height: 3,
            channels: 4,
            pixels: PixelData::F32(vec![]),
        };
        assert!(!LinearImage::from_decoded(&empty, 1.0).is_valid());
    }
}
