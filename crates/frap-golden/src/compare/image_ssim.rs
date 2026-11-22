// SPDX-License-Identifier: GPL-3.0-or-later
//! `image_ssim` (T4, §20.4): SSIM ≥ threshold plus a mean ΔE limit in a centre crop.
//!
//! Thresholds: against frap-blessed self-goldens [`SELF_MIN_SSIM`] (gating from M1); Workbench
//! against Blender [`WORKBENCH_MIN_SSIM`] (reported in M1, gating from M2); EEVEE against
//! Blender [`EEVEE_MIN_SSIM`] (gating from M5, raised over time).
//!
//! Pixels are compared as 8-bit sRGB-encoded RGB composited over black (alpha is compared as a
//! separate grey channel when either image is not opaque). SSIM is `image-compare`'s
//! `MSSIMSimple` (8×8 windows, minimum over channels); ΔE is CIE76 in CIELAB (D65).

use frap_imbuf::{DecodedImage, PixelData};
use image_compare::prelude::{GrayImage, RgbImage};

use crate::case::Mismatch;

/// Comparator name.
pub const NAME: &str = "image_ssim";
/// frap against frap-blessed images.
pub const SELF_MIN_SSIM: f64 = 0.995;
/// Workbench against Blender Workbench.
pub const WORKBENCH_MIN_SSIM: f64 = 0.97;
/// EEVEE-like against Blender EEVEE.
pub const EEVEE_MIN_SSIM: f64 = 0.90;
/// Default mean ΔE limit in the centre crop (about one just-noticeable difference).
pub const DEFAULT_MAX_DELTA_E: f64 = 2.0;
/// Centre crop: this fraction of width and height.
pub const CENTRE_CROP: f64 = 0.5;

/// Decodes a PNG (palette and low bit depths expanded; 16-bit kept).
pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().map_err(|e| format!("PNG: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG: image too large")?];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG: {e}"))?;
    buf.truncate(info.buffer_size());
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("PNG: palette not expanded".into()),
    };
    let pixels = match info.bit_depth {
        png::BitDepth::Sixteen => PixelData::U16(
            buf.chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect(),
        ),
        png::BitDepth::Eight => PixelData::U8(buf),
        d => return Err(format!("PNG: unexpected bit depth {d:?}")),
    };
    Ok(DecodedImage {
        width: info.width,
        height: info.height,
        channels,
        pixels,
    })
}

/// Reads and decodes a PNG file.
pub fn load_png(path: &std::path::Path) -> Result<DecodedImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    decode_png(&bytes)
}

/// RGBA8 pixels of any decoded image (16-bit rounded; float clamped to [0, 1] and taken as
/// already encoded; grey expanded; missing alpha = 255).
pub fn to_rgba8(img: &DecodedImage) -> Vec<u8> {
    let n = img.width as usize * img.height as usize;
    let ch = img.channels.clamp(1, 4) as usize;
    let sample = |i: usize| -> u8 {
        match &img.pixels {
            PixelData::U8(v) => v.get(i).copied().unwrap_or(0),
            PixelData::U16(v) => v.get(i).map_or(0, |x| ((*x as u32 + 128) / 257) as u8),
            PixelData::F32(v) => v
                .get(i)
                .map_or(0, |x| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8),
            _ => 0,
        }
    };
    let mut out = Vec::with_capacity(n * 4);
    for p in 0..n {
        let b = p * ch;
        let (r, g, bl, a) = match ch {
            1 => (sample(b), sample(b), sample(b), 255),
            2 => (sample(b), sample(b), sample(b), sample(b + 1)),
            3 => (sample(b), sample(b + 1), sample(b + 2), 255),
            _ => (sample(b), sample(b + 1), sample(b + 2), sample(b + 3)),
        };
        out.extend_from_slice(&[r, g, bl, a]);
    }
    out
}

/// Result of comparing two images.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ImageScore {
    /// SSIM (1 = identical), minimum over RGB (and alpha when present).
    pub ssim: f64,
    /// Mean CIE76 ΔE in the centre crop.
    pub mean_delta_e: f64,
}

fn srgb_to_lab(c: [u8; 3]) -> [f64; 3] {
    let lin = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = c.map(lin);
    let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIE76 ΔE between two sRGB colours.
pub fn delta_e(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (p, q) = (srgb_to_lab(a), srgb_to_lab(b));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

fn ssim_channels(w: u32, h: u32, a: &[u8], b: &[u8], alpha: bool) -> Result<f64, String> {
    let rgb = |p: &[u8]| -> Vec<u8> {
        p.chunks_exact(4)
            .flat_map(|c| {
                let a = c[3] as u32;
                [0, 1, 2].map(|k| ((c[k] as u32 * a + 127) / 255) as u8)
            })
            .collect()
    };
    if w < 8 || h < 8 {
        // Too small for 8×8 windows: 1 − normalised RMS error.
        let (ra, rb) = (rgb(a), rgb(b));
        let mse: f64 = ra
            .iter()
            .zip(&rb)
            .map(|(x, y)| ((*x as f64 - *y as f64) / 255.0).powi(2))
            .sum::<f64>()
            / ra.len().max(1) as f64;
        return Ok(1.0 - mse.sqrt());
    }
    let ia = RgbImage::from_raw(w, h, rgb(a)).ok_or("bad RGB buffer")?;
    let ib = RgbImage::from_raw(w, h, rgb(b)).ok_or("bad RGB buffer")?;
    let mut score =
        image_compare::rgb_similarity_structure(&image_compare::Algorithm::MSSIMSimple, &ia, &ib)
            .map_err(|e| e.to_string())?
            .score;
    if alpha {
        let al = |p: &[u8]| -> Vec<u8> { p.chunks_exact(4).map(|c| c[3]).collect() };
        let ga = GrayImage::from_raw(w, h, al(a)).ok_or("bad alpha buffer")?;
        let gb = GrayImage::from_raw(w, h, al(b)).ok_or("bad alpha buffer")?;
        let s = image_compare::gray_similarity_structure(
            &image_compare::Algorithm::MSSIMSimple,
            &ga,
            &gb,
        )
        .map_err(|e| e.to_string())?
        .score;
        score = score.min(s);
    }
    Ok(score)
}

/// Scores `frap` against `reference`. Errors when the sizes differ.
pub fn score(frap: &DecodedImage, reference: &DecodedImage) -> Result<ImageScore, String> {
    if (frap.width, frap.height) != (reference.width, reference.height) {
        return Err(format!(
            "size {}x{} != {}x{}",
            frap.width, frap.height, reference.width, reference.height
        ));
    }
    let (w, h) = (frap.width, frap.height);
    let (a, b) = (to_rgba8(frap), to_rgba8(reference));
    let alpha = a.chunks_exact(4).any(|c| c[3] != 255) || b.chunks_exact(4).any(|c| c[3] != 255);
    let ssim = if a == b {
        1.0
    } else {
        ssim_channels(w, h, &a, &b, alpha)?
    };
    let (cw, ch) = (
        ((w as f64 * CENTRE_CROP).round() as u32).max(1),
        ((h as f64 * CENTRE_CROP).round() as u32).max(1),
    );
    let (x0, y0) = ((w - cw.min(w)) / 2, (h - ch.min(h)) / 2);
    let mut sum = 0.0;
    let mut n = 0usize;
    for y in y0..(y0 + ch).min(h) {
        for x in x0..(x0 + cw).min(w) {
            let i = (y as usize * w as usize + x as usize) * 4;
            sum += delta_e([a[i], a[i + 1], a[i + 2]], [b[i], b[i + 1], b[i + 2]]);
            n += 1;
        }
    }
    Ok(ImageScore {
        ssim,
        mean_delta_e: if n == 0 { 0.0 } else { sum / n as f64 },
    })
}

/// Compares `frap` with `reference`: SSIM ≥ `min_ssim` and mean ΔE ≤ `max_delta_e`.
pub fn compare(
    what: &str,
    frap: &DecodedImage,
    reference: &DecodedImage,
    min_ssim: f64,
    max_delta_e: f64,
) -> Vec<Mismatch> {
    match score(frap, reference) {
        Err(e) => vec![Mismatch::new(NAME, what, e)],
        Ok(s) => {
            let mut out = Vec::new();
            if s.ssim < min_ssim {
                out.push(Mismatch::new(
                    NAME,
                    what,
                    format!("SSIM {:.5} < {min_ssim}", s.ssim),
                ));
            }
            if s.mean_delta_e > max_delta_e {
                out.push(Mismatch::new(
                    NAME,
                    what,
                    format!("mean ΔE {:.3} > {max_delta_e}", s.mean_delta_e),
                ));
            }
            out
        }
    }
}

/// `frap` with every 8-bit channel within one level of `reference`'s taken from `reference`
/// (`None` when the sizes differ). For self-goldens across GPU adapters: float evaluation
/// differs in its last bits between drivers (transcendental functions, FMA contraction), which
/// moves the 8-bit rounding contours of smooth gradients by one pixel; on a flat region an
/// 8 × 8 SSIM window with such a contour scores about 0.9, so a handful of contours fails the
/// 0.995 gate although no pixel moved by more than 1/255 (decision log M56CI-6).
pub fn snap_one_level(frap: &DecodedImage, reference: &DecodedImage) -> Option<DecodedImage> {
    if (frap.width, frap.height) != (reference.width, reference.height) {
        return None;
    }
    let (mut a, b) = (to_rgba8(frap), to_rgba8(reference));
    for (x, y) in a.iter_mut().zip(&b) {
        if x.abs_diff(*y) <= 1 {
            *x = *y;
        }
    }
    Some(DecodedImage {
        width: frap.width,
        height: frap.height,
        channels: 4,
        pixels: PixelData::U8(a),
    })
}

/// [`compare`] after [`snap_one_level`]: differences of one 8-bit level are not counted.
pub fn compare_within_one_level(
    what: &str,
    frap: &DecodedImage,
    reference: &DecodedImage,
    min_ssim: f64,
    max_delta_e: f64,
) -> Vec<Mismatch> {
    match snap_one_level(frap, reference) {
        Some(snapped) => compare(what, &snapped, reference, min_ssim, max_delta_e),
        None => compare(what, frap, reference, min_ssim, max_delta_e),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A 32×32 RGB gradient with a dark square.
    pub(crate) fn picture() -> DecodedImage {
        let (w, h) = (32u32, 32u32);
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let dark = (8..16).contains(&x) && (8..16).contains(&y);
                if dark {
                    px.extend_from_slice(&[10, 10, 10]);
                } else {
                    px.extend_from_slice(&[(x * 8) as u8, (y * 8) as u8, 128]);
                }
            }
        }
        DecodedImage {
            width: w,
            height: h,
            channels: 3,
            pixels: PixelData::U8(px),
        }
    }

    pub(crate) fn encode_png(img: &DecodedImage) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, img.width, img.height);
            enc.set_color(match img.channels {
                1 => png::ColorType::Grayscale,
                2 => png::ColorType::GrayscaleAlpha,
                3 => png::ColorType::Rgb,
                _ => png::ColorType::Rgba,
            });
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            let PixelData::U8(p) = &img.pixels else {
                panic!("8-bit only")
            };
            w.write_image_data(p).unwrap();
        }
        out
    }

    #[test]
    fn identical_images_score_one() {
        let a = picture();
        let s = score(&a, &a.clone()).unwrap();
        assert_eq!(s.ssim, 1.0);
        assert_eq!(s.mean_delta_e, 0.0);
        assert!(compare("img", &a, &a, SELF_MIN_SSIM, DEFAULT_MAX_DELTA_E).is_empty());
    }

    #[test]
    fn equal_under_re_encoding() {
        // Same pixels as RGBA8, as 16-bit RGB, and through a PNG round trip.
        let a = picture();
        let PixelData::U8(p) = &a.pixels else {
            unreachable!()
        };
        let rgba = DecodedImage {
            channels: 4,
            pixels: PixelData::U8(
                p.chunks_exact(3)
                    .flat_map(|c| [c[0], c[1], c[2], 255])
                    .collect(),
            ),
            ..a.clone()
        };
        let wide = DecodedImage {
            pixels: PixelData::U16(p.iter().map(|x| *x as u16 * 257).collect()),
            ..a.clone()
        };
        let png = decode_png(&encode_png(&a)).unwrap();
        for other in [rgba, wide, png] {
            assert_eq!(score(&a, &other).unwrap().ssim, 1.0);
        }
    }

    #[test]
    fn differs_under_one_change() {
        let a = picture();
        let mut b = a.clone();
        let PixelData::U8(p) = &mut b.pixels else {
            unreachable!()
        };
        // One pixel in the centre crop.
        let i = (16 * 32 + 16) * 3;
        p[i] = 255;
        p[i + 1] = 0;
        p[i + 2] = 0;
        let s = score(&a, &b).unwrap();
        assert!(s.ssim < 1.0, "{s:?}");
        assert!(s.mean_delta_e > 0.0);
        // An exact threshold catches it; the default self-golden threshold tolerates it.
        assert_eq!(compare("img", &b, &a, 1.0, 0.0).len(), 2);
        // A large change fails the default thresholds.
        let mut c = a.clone();
        let PixelData::U8(p) = &mut c.pixels else {
            unreachable!()
        };
        for x in p.iter_mut().skip(1).step_by(3) {
            *x = 255 - *x;
        }
        assert!(!compare("img", &c, &a, SELF_MIN_SSIM, DEFAULT_MAX_DELTA_E).is_empty());
    }

    #[test]
    fn size_mismatch_and_alpha() {
        let a = picture();
        let small = DecodedImage {
            width: 4,
            height: 4,
            channels: 1,
            pixels: PixelData::U8(vec![0; 16]),
        };
        assert_eq!(compare("img", &a, &small, 0.9, 10.0).len(), 1);
        let t = |alpha: u8| DecodedImage {
            width: 16,
            height: 16,
            channels: 4,
            pixels: PixelData::U8(
                (0..256)
                    .flat_map(|i| [100, 100, 100, if i % 3 == 0 { alpha } else { 255 }])
                    .collect(),
            ),
        };
        assert!(score(&t(0), &t(255)).unwrap().ssim < 0.99);
    }

    #[test]
    fn delta_e_reference_values() {
        assert_eq!(delta_e([10, 20, 30], [10, 20, 30]), 0.0);
        // Black to white is L* 0 → 100.
        assert!((delta_e([0, 0, 0], [255, 255, 255]) - 100.0).abs() < 0.01);
    }

    #[test]
    fn one_level_contours_are_not_counted() {
        let a = picture();
        let PixelData::U8(px) = &a.pixels else {
            unreachable!()
        };
        // A one-level contour over a flat region fails the self-golden gate...
        let mut flat = vec![100u8; 32 * 32 * 3];
        let mut contour = flat.clone();
        for y in 0..32 {
            for x in 0..32 {
                if x + y == 31 || x + y == 32 {
                    contour[(y * 32 + x) * 3] = 101;
                }
            }
        }
        let img = |p: Vec<u8>| DecodedImage {
            width: 32,
            height: 32,
            channels: 3,
            pixels: PixelData::U8(p),
        };
        assert!(
            !compare(
                "c",
                &img(contour.clone()),
                &img(flat.clone()),
                SELF_MIN_SSIM,
                2.0
            )
            .is_empty()
        );
        // ...but not the one-level comparison; a two-level difference still does.
        assert!(
            compare_within_one_level("c", &img(contour), &img(flat.clone()), SELF_MIN_SSIM, 2.0)
                .is_empty()
        );
        for y in 0..32 {
            for x in 0..32 {
                if x + y == 31 || x + y == 32 {
                    flat[(y * 32 + x) * 3] = 102;
                }
            }
        }
        assert!(
            !compare_within_one_level(
                "c",
                &img(flat),
                &img(vec![100u8; 32 * 32 * 3]),
                SELF_MIN_SSIM,
                2.0
            )
            .is_empty()
        );
        // The picture itself is unchanged by snapping onto itself.
        let s = snap_one_level(&a, &a).unwrap();
        assert_eq!(score(&s, &a).unwrap().ssim, 1.0);
        assert_eq!(px.len(), 32 * 32 * 3);
    }
}
