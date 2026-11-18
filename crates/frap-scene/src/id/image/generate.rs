// SPDX-License-Identifier: GPL-3.0-or-later
//! Generated image buffers (`Image.source == 'GENERATED'`): `BLANK`, `UV_GRID`, `COLOR_GRID`.
//!
//! Ported from blender/source/blender/blenkernel/intern/image_gen.cc (`BKE_image_buf_fill_color`,
//! `BKE_image_buf_fill_checker`, `BKE_image_buf_fill_checker_color`) and `add_ibuf_for_tile`
//! (`image.cc`), Blender 5.2.4, GPL-2.0-or-later. Rows are bottom first, as in Blender.
//!
//! Deviation (docs/deviations.md): `COLOR_GRID` has no letter/number labels. Blender draws them
//! with its monospace UI font (`checker_board_text`, BLF at 54 px, every 128 pixels from
//! (33, 44)); frap has no font rasteriser. Every other pixel is Blender's.

use super::imbuf::{ImBuf, ImBufData, hsv_to_rgb, srgb_to_linearrgb, unit_float_to_uchar_clamp};

/// `BKE_image_buf_fill_color`: every pixel `color` (bytes through `rgba_float_to_uchar`).
pub fn fill_color(buf: &mut ImBuf, color: [f32; 4]) {
    match &mut buf.data {
        ImBufData::Float(f) => {
            for px in f.chunks_exact_mut(4) {
                px.copy_from_slice(&color);
            }
        }
        ImBufData::Byte(b) => {
            let c = color.map(unit_float_to_uchar_clamp);
            for px in b.chunks_exact_mut(4) {
                px.copy_from_slice(&c);
            }
        }
    }
}

/// `char(v * 255.0f)` stored into a `uchar` (truncation toward zero, as the C conversion).
#[inline]
fn char255(v: f32) -> u8 {
    (v * 255.0) as i32 as u8
}

/// `BKE_image_buf_fill_checker` (`UV_GRID`).
pub fn fill_checker(buf: &mut ImBuf) {
    const CHECKER: i32 = 32;
    const HALF: i32 = CHECKER / 2;
    let (width, height) = (buf.x as i32, buf.y as i32);
    let dark_linear = srgb_to_linearrgb(0.25);
    let bright_linear = srgb_to_linearrgb(0.58);
    // Checkers.
    for y in 0..height {
        let mut dark: i32 = if (y / CHECKER) % 2 == 0 { 1 } else { -1 };
        for x in 0..width {
            if x % CHECKER == 0 {
                dark = -dark;
            }
            let i = ((y * width + x) * 4) as usize;
            match &mut buf.data {
                ImBufData::Float(f) => {
                    let v = if dark > 0 { dark_linear } else { bright_linear };
                    f[i..i + 4].copy_from_slice(&[v, v, v, 1.0]);
                }
                ImBufData::Byte(b) => {
                    let v = if dark > 0 { 64 } else { 150 };
                    b[i..i + 4].copy_from_slice(&[v, v, v, 255]);
                }
            }
        }
    }
    // Second pass: coloured `+`.
    for y in 0..height {
        let hoffs = 0.125f32 * (y / CHECKER) as f32;
        for x in 0..width {
            let h = 0.125f32 * (x / CHECKER) as f32;
            let test_x = ((x % CHECKER) - HALF).abs();
            let test_y = ((y % CHECKER) - HALF).abs();
            if test_x < 4 && test_y < 4 && (test_x < 1 || test_y < 1) {
                let rgb = hsv_to_rgb((h - hoffs).abs() % 1.0, 0.9, 0.9);
                let i = ((y * width + x) * 4) as usize;
                match &mut buf.data {
                    ImBufData::Byte(b) => {
                        b[i..i + 4].copy_from_slice(&[
                            char255(rgb[0]),
                            char255(rgb[1]),
                            char255(rgb[2]),
                            255,
                        ]);
                    }
                    ImBufData::Float(f) => {
                        // `IMB_colormanagement_srgb_to_scene_linear_v3` (factory config: the
                        // sRGB EOTF).
                        f[i..i + 4].copy_from_slice(&[
                            srgb_to_linearrgb(rgb[0]),
                            srgb_to_linearrgb(rgb[1]),
                            srgb_to_linearrgb(rgb[2]),
                            1.0,
                        ]);
                    }
                }
            }
        }
    }
}

/// `power_of_2_max_i`
fn power_of_2_max_i(mut n: i32) -> i32 {
    let is_pow2 = |n: i32| n & (n - 1) == 0;
    if is_pow2(n) {
        return n;
    }
    loop {
        n &= n - 1;
        if is_pow2(n) {
            return n * 2;
        }
    }
}

/// `BLEND_CHAR(real, add)`
#[inline]
fn blend_char(real: u8, add: f32) -> u8 {
    let a = i32::from(char255(add));
    let s = i32::from(real) + a;
    if s <= 255 { s as u8 } else { 255 }
}

/// `BLEND_FLOAT(real, add)`
#[inline]
fn blend_float(real: f32, add: f32) -> f32 {
    if real + add <= 1.0 { real + add } else { 1.0 }
}

/// `checker_board_color_fill`
fn color_fill(buf: &mut ImBuf) {
    let (width, height) = (buf.x as i32, buf.y as i32);
    let hue_step = power_of_2_max_i(width / 8).max(8);
    for y in 0..height {
        // Use a number lower than 1.0 else its too bright.
        let v = (0.1 + (f64::from(y) * (0.4 / f64::from(height)))) as f32;
        for x in 0..width {
            let h = (f64::from(x / hue_step) * 1.0 / f64::from(width) * f64::from(hue_step)) as f32;
            let rgb = hsv_to_rgb(h, 1.0, v);
            let i = ((y * width + x) * 4) as usize;
            match &mut buf.data {
                ImBufData::Byte(b) => {
                    b[i..i + 4].copy_from_slice(&[
                        char255(rgb[0]),
                        char255(rgb[1]),
                        char255(rgb[2]),
                        255,
                    ]);
                }
                // `IMB_colormanagement_rec709_to_scene_linear`: identity in the factory config.
                ImBufData::Float(f) => f[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 1.0]),
            }
        }
    }
}

/// `checker_board_color_tint`
fn color_tint(buf: &mut ImBuf, size: i32, blend: f32) {
    let (width, height) = (buf.x as i32, buf.y as i32);
    let blend_half = blend * 0.5;
    for y in 0..height {
        for x in 0..width {
            let even = ((y / size) % 2 == 1 && (x / size) % 2 == 1)
                || ((y / size) % 2 == 0 && (x / size) % 2 == 0);
            let add = if even { blend } else { blend_half };
            let i = ((y * width + x) * 4) as usize;
            match &mut buf.data {
                ImBufData::Byte(b) => {
                    for c in 0..3 {
                        b[i + c] = blend_char(b[i + c], add);
                    }
                    b[i + 3] = 255;
                }
                ImBufData::Float(f) => {
                    for c in 0..3 {
                        f[i + c] = blend_float(f[i + c], add);
                    }
                    f[i + 3] = 1.0;
                }
            }
        }
    }
}

/// `checker_board_grid_fill`
fn grid_fill(buf: &mut ImBuf, blend: f32) {
    let (width, height) = (buf.x as i32, buf.y as i32);
    for y in 0..height {
        for x in 0..width {
            if y % 32 == 0 || x % 32 == 0 || x == 0 {
                let i = ((y * width + x) * 4) as usize;
                match &mut buf.data {
                    ImBufData::Byte(b) => {
                        for c in 0..3 {
                            b[i + c] = blend_char(b[i + c], blend);
                        }
                        b[i + 3] = 255;
                    }
                    ImBufData::Float(f) => {
                        for c in 0..3 {
                            f[i + c] = blend_float(f[i + c], blend);
                        }
                        f[i + 3] = 1.0;
                    }
                }
            }
        }
    }
}

/// `BKE_image_buf_fill_checker_color` (`COLOR_GRID`, without the labels: see the module
/// documentation).
pub fn fill_checker_color(buf: &mut ImBuf) {
    color_fill(buf);
    color_tint(buf, 1, 0.03);
    color_tint(buf, 4, 0.05);
    color_tint(buf, 32, 0.07);
    color_tint(buf, 128, 0.15);
    grid_fill(buf, 1.0 / 4.0);
    // `IMB_buffer_float_rgba_srgb_to_linear`: the float buffer was filled in sRGB.
    if let ImBufData::Float(f) = &mut buf.data {
        for px in f.chunks_exact_mut(4) {
            for c in &mut px[..3] {
                *c = srgb_to_linearrgb(*c);
            }
        }
    }
}

/// Generation parameters of an image's first tile (`ImageTile.gen_*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenParams<'a> {
    /// `gen_type`: `BLANK`, `UV_GRID`, `COLOR_GRID`.
    pub kind: &'a str,
    /// `gen_x`
    pub width: i32,
    /// `gen_y`
    pub height: i32,
    /// `gen_flag & IMA_GEN_FLOAT`
    pub float: bool,
    /// `gen_depth` (24 or 32).
    pub depth: u8,
    /// `gen_color`
    pub color: [f32; 4],
    /// The image's colour space is a data space (`Non-Color`): the float fill colour is not
    /// linearised.
    pub is_data: bool,
}

/// `add_ibuf_for_tile`: the buffer of a generated image.
pub fn generate(p: &GenParams<'_>) -> ImBuf {
    let (w, h) = (p.width.max(1) as u32, p.height.max(1) as u32);
    let mut buf = ImBuf::new(w, h, p.depth, p.float);
    match p.kind {
        "UV_GRID" => fill_checker(&mut buf),
        "COLOR_GRID" => fill_checker_color(&mut buf),
        _ => {
            let mut c = p.color;
            if p.float && !p.is_data {
                // "The input color here should ideally be linear already, but for now we just
                // convert and postpone breaking the API for later."
                for v in &mut c[..3] {
                    *v = srgb_to_linearrgb(*v);
                }
            }
            fill_color(&mut buf, c);
        }
    }
    buf
}

#[cfg(test)]
mod tests;
