// SPDX-License-Identifier: GPL-3.0-or-later
//! Radiance HDR, byte for byte as Blender 5.2.4 writes it (`format_hdr.cc` → OpenImageIO 3.1).
//!
//! Ported from OpenImageIO 3.1.13 `src/hdr.imageio/hdroutput.cpp` (Apache-2.0, Copyright
//! Contributors to the OpenImageIO project; itself based on Bruce Walter's public-domain
//! `rgbe.c`): the header `#?RADIANCE`, `FORMAT=32-bit_rle_rgbe`, `-Y h +X w`; `float2rgbe`
//! (`frexpf` of the largest component, truncating conversions); rows top first, run-length
//! encoded per channel (the "new" Radiance RLE) for widths 8..=32767, flat otherwise. Grey
//! expands to RGB and alpha is dropped (Blender's writer always asks for three channels).

use crate::codec::{DecodedImage, ImageError};

pub(crate) fn encode_hdr(image: &DecodedImage) -> Result<Vec<u8>, ImageError> {
    let rgb = image.with_channels(3)?.to_f32();
    let (w, h) = (image.width as usize, image.height as usize);
    let mut out = Vec::with_capacity(64 + w * h * 4);
    out.extend_from_slice(b"#?RADIANCE\n");
    out.extend_from_slice(b"FORMAT=32-bit_rle_rgbe\n\n");
    out.extend_from_slice(format!("-Y {h} +X {w}\n").as_bytes());
    for row in rgb.chunks_exact(w * 3) {
        write_pixels_rle(&mut out, row, w);
    }
    Ok(out)
}

/// `frexpf` for the values `float2rgbe` passes (≥ 1e-32, so normal; infinities and NaN return
/// themselves with exponent 0, as C libraries do).
fn frexpf(v: f32) -> (f32, i32) {
    if !v.is_finite() || v == 0.0 {
        return (v, 0);
    }
    let bits = v.to_bits();
    let exp = ((bits >> 23) & 0xff) as i32;
    if exp == 0 {
        // Subnormal: scale into the normal range first.
        let (m, e) = frexpf(v * f32::from_bits(0x4c00_0000)); // 2^25
        return (m, e - 25);
    }
    let m = f32::from_bits((bits & 0x807f_ffff) | (126 << 23));
    (m, exp - 126)
}

/// `float2rgbe`.
fn float2rgbe(red: f32, green: f32, blue: f32) -> [u8; 4] {
    let mut v = red;
    if green > v {
        v = green;
    }
    if blue > v {
        v = blue;
    }
    // `v < 1e-32` compares the float with a double literal.
    if f64::from(v) < 1e-32 {
        [0; 4]
    } else {
        let (m, e) = frexpf(v);
        let v = m * 256.0 / v;
        // C's float → unsigned char conversions truncate; Rust's `as` saturates out-of-range
        // values (negative components) to 0 as arm64 does.
        [
            (red * v) as u8,
            (green * v) as u8,
            (blue * v) as u8,
            (e + 128) as u8,
        ]
    }
}

/// `RGBE_WritePixels_RLE` for one scanline.
fn write_pixels_rle(out: &mut Vec<u8>, data: &[f32], width: usize) {
    if !(8..=0x7fff).contains(&width) {
        for p in data.chunks_exact(3) {
            out.extend_from_slice(&float2rgbe(p[0], p[1], p[2]));
        }
        return;
    }
    out.extend_from_slice(&[2, 2, (width >> 8) as u8, (width & 0xff) as u8]);
    let mut buffer = vec![0u8; width * 4];
    for (i, p) in data.chunks_exact(3).enumerate() {
        let rgbe = float2rgbe(p[0], p[1], p[2]);
        for (c, v) in rgbe.into_iter().enumerate() {
            buffer[i + c * width] = v;
        }
    }
    for c in 0..4 {
        write_bytes_rle(out, &buffer[c * width..(c + 1) * width]);
    }
}

/// `RGBE_WriteBytes_RLE`: runs of at least 4 equal bytes, literal runs of at most 128.
fn write_bytes_rle(out: &mut Vec<u8>, data: &[u8]) {
    const MINRUNLENGTH: usize = 4;
    let numbytes = data.len();
    let mut cur = 0;
    while cur < numbytes {
        let mut beg_run = cur;
        let mut run_count = 0;
        let mut old_run_count = 0;
        while run_count < MINRUNLENGTH && beg_run < numbytes {
            beg_run += run_count;
            old_run_count = run_count;
            run_count = 1;
            while beg_run + run_count < numbytes
                && run_count < 127
                && data[beg_run] == data[beg_run + run_count]
            {
                run_count += 1;
            }
        }
        if old_run_count > 1 && old_run_count == beg_run - cur {
            out.extend_from_slice(&[(128 + old_run_count) as u8, data[cur]]);
            cur = beg_run;
        }
        while cur < beg_run {
            let nonrun_count = (beg_run - cur).min(128);
            out.push(nonrun_count as u8);
            out.extend_from_slice(&data[cur..cur + nonrun_count]);
            cur += nonrun_count;
        }
        if run_count >= MINRUNLENGTH {
            out.extend_from_slice(&[(128 + run_count) as u8, data[beg_run]]);
            cur += run_count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frexpf_matches_c() {
        assert_eq!(frexpf(1.0), (0.5, 1));
        assert_eq!(frexpf(0.75), (0.75, 0));
        assert_eq!(frexpf(9.0), (0.5625, 4));
        assert_eq!(frexpf(-3.0), (-0.75, 2));
        for v in [1e-40f32, 3.0e-39, 1e-32, 123.456, 65504.0] {
            let (m, e) = frexpf(v);
            assert!((0.5..1.0).contains(&m), "{v}: {m}");
            assert_eq!(f64::from(m) * 2f64.powi(e), f64::from(v), "{v}");
        }
        assert_eq!(frexpf(f32::INFINITY), (f32::INFINITY, 0));
    }

    #[test]
    fn rgbe_of_simple_values() {
        assert_eq!(float2rgbe(1.0, 0.5, 0.0), [128, 64, 0, 129]);
        assert_eq!(float2rgbe(0.0, 0.0, 1e-33), [0; 4]);
        assert_eq!(float2rgbe(-1.0, 0.25, 0.0), [0, 128, 0, 127]);
    }
}
