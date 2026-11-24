// SPDX-License-Identifier: GPL-3.0-or-later
//! The PNG writer of `uv.export_layout`: what `export_uv_png.export` draws on the GPU, drawn
//! on the CPU so that it needs no adapter (Blender's `blender -b` needs `gpu.init()` for it).
//!
//! Ported from blender/scripts/addons_core/io_mesh_uv_layout/export_uv_png.py, the
//! `POLYLINE_UNIFORM_COLOR` shader (blender/source/blender/gpu/shaders/
//! gpu_shader_3D_polyline_{vert,frag}.glsl: each line a quad two pixels wide, alpha
//! `clamp(1 - |distance|, 0, 1)`) and `GPU_BLEND_ALPHA`, Blender 5.2.4, GPL-2.0-or-later:
//! - an RGBA8 framebuffer cleared to (0, 0, 0, 0); the faces first (`FLAT_COLOR`, their colour
//!   with alpha `opacity`, triangulated, sampled at pixel centres with a top-left rule), then
//!   the edges (black, smooth), every fragment blended `src·a + dst·(1 - a)` (alpha:
//!   `a + dst_a·(1 - a)`) and stored as 8 bits;
//! - written as the OpenImageIO PNG writer does: top row first (v = 1 at the top), colour
//!   un-premultiplied by alpha, RGBA 8 bits, deflated at zlib level 6 (imbuf's default PNG
//!   `compress` of 15 % → `int(15 / 11.1111)` would be 1; OIIO's writer, which the add-on's
//!   `image.save()` reaches, uses zlib's default level 6).
//!
//! The framebuffer is `width × height × 4` bytes: [`buffer_bytes`] is checked against
//! `--max-elements` before it is allocated (`LIMIT_EXCEEDED`, like `images.new`), and the
//! rasteriser and the encoder are cancellation points of `--stmt-timeout` (per scanline).
//!
//! The image is compared by SSIM with Blender's (`image_ssim`), not byte for byte.

use super::vector::Poly;

use std::io::Write as _;

use frap_math::budget;

/// The `what` of the framebuffer's `LIMIT_EXCEEDED`.
pub(crate) const BUFFER: &str = "uv.export_layout image";

/// The bytes of the RGBA8 framebuffer of a `width × height` export (`None`: it overflows).
pub(crate) fn buffer_bytes(width: i32, height: i32) -> Option<u64> {
    let (w, h) = (
        u64::from(width.max(1).unsigned_abs()),
        u64::from(height.max(1).unsigned_abs()),
    );
    w.checked_mul(h)?.checked_mul(4)
}

/// An RGBA8 premultiplied framebuffer, rows from the top, four bytes per pixel.
struct Fb {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

/// Float to unorm8 (round to nearest).
#[inline]
fn unorm8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

impl Fb {
    /// `GPU_BLEND_ALPHA` of one fragment.
    fn blend(&mut self, x: usize, y: usize, rgb: [f32; 3], a: f32) {
        let i = (y * self.w + x) * 4;
        let d = &mut self.px[i..i + 4];
        let inv = 1.0 - a;
        let c = |s: f32, dv: u8| s * a + f32::from(dv) / 255.0 * inv;
        let out = [
            unorm8(c(rgb[0], d[0])),
            unorm8(c(rgb[1], d[1])),
            unorm8(c(rgb[2], d[2])),
            unorm8(a + f32::from(d[3]) / 255.0 * inv),
        ];
        d.copy_from_slice(&out);
    }

    /// The pixels whose centre lies inside triangle `t` (top-left rule), in raster order.
    fn triangle(&self, t: [[f32; 2]; 3], mut f: impl FnMut(usize, usize, f64, f64)) {
        let p = t.map(|v| [f64::from(v[0]), f64::from(v[1])]);
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if area == 0.0 || !area.is_finite() {
            return;
        }
        // Orient counter-clockwise in a y-down raster (area < 0 here).
        let p = if area > 0.0 { [p[0], p[2], p[1]] } else { p };
        let edges = [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])];
        let xmin = p.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min);
        let xmax = p.iter().map(|v| v[0]).fold(f64::NEG_INFINITY, f64::max);
        let ymin = p.iter().map(|v| v[1]).fold(f64::INFINITY, f64::min);
        let ymax = p.iter().map(|v| v[1]).fold(f64::NEG_INFINITY, f64::max);
        let x0 = (xmin - 0.5).floor().max(0.0) as usize;
        let y0 = (ymin - 0.5).floor().max(0.0) as usize;
        let x1 = ((xmax - 0.5).ceil() as i64).min(self.w as i64 - 1);
        let y1 = ((ymax - 0.5).ceil() as i64).min(self.h as i64 - 1);
        if x1 < 0 || y1 < 0 {
            return;
        }
        let span = u32::try_from(x1 as usize + 1 - x0.min(x1 as usize + 1)).unwrap_or(u32::MAX);
        for y in y0..=y1 as usize {
            // `--stmt-timeout`: one cancellation point per scanline.
            budget::check_n(span);
            let cy = y as f64 + 0.5;
            for x in x0..=x1 as usize {
                let cx = x as f64 + 0.5;
                let inside = edges.iter().all(|&(a, b)| {
                    let e = (b[0] - a[0]) * (cy - a[1]) - (b[1] - a[1]) * (cx - a[0]);
                    // Top edge (horizontal, going left) or left edge (going down).
                    let top_left = (a[1] == b[1] && b[0] < a[0]) || b[1] > a[1];
                    if top_left { e <= 0.0 } else { e < 0.0 }
                });
                if inside {
                    f(x, y, cx, cy);
                }
            }
        }
    }
}

/// `get_normalize_uvs_matrix` and the viewport transform: UV to pixels (y down, OIIO).
fn to_px(uv: [f64; 2], tile: (i64, i64), w: f32, h: f32) -> [f32; 2] {
    let (u, v) = (uv[0] as f32, uv[1] as f32);
    let ndc_x = 2.0 * u + (-1.0 - tile.0 as f32 * 2.0);
    let ndc_y = -2.0 * v + -(-1.0 - tile.1 as f32 * 2.0);
    // Window y grows upwards; the PNG's first row is the framebuffer's first row.
    [(ndc_x + 1.0) * 0.5 * w, (ndc_y + 1.0) * 0.5 * h]
}

/// `export_uv_png.export`: the RGBA8 image (un-premultiplied, top row first). Unwinds with
/// `LIMIT_EXCEEDED` when the image is over `--max-elements` ([`buffer_bytes`]) and with
/// `TIMEOUT` past the statement's deadline.
pub(crate) fn draw(
    tile: (i64, i64),
    polys: &[Poly],
    width: i32,
    height: i32,
    opacity: f64,
) -> Vec<u8> {
    budget::check_buffer(BUFFER, buffer_bytes(width, height));
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let mut fb = Fb {
        w,
        h,
        px: vec![0; w * h * 4],
    };
    let (wf, hf) = (w as f32, h as f32);
    let alpha = opacity as f32;
    // `draw_background_colors`: every face triangulated, in one batch.
    for p in polys {
        let pts: Vec<[f32; 2]> = p.uvs.iter().map(|&uv| to_px(uv, tile, wf, hf)).collect();
        let rgb = p.color.map(|c| c as f32);
        let uv32: Vec<[f32; 2]> = p
            .uvs
            .iter()
            .map(|uv| [uv[0] as f32, uv[1] as f32])
            .collect();
        let tris = if uv32.len() < 3 {
            Vec::new()
        } else {
            frap_geom::polyfill::polyfill_calc(&uv32, 0)
        };
        budget::check_now();
        for t in tris {
            let tri = [pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]];
            let mut frags = Vec::new();
            fb.triangle(tri, |x, y, _, _| frags.push((x, y)));
            for (x, y) in frags {
                fb.blend(x, y, rgb, alpha);
            }
        }
    }
    // `draw_lines`: every edge of every face, black, smooth, one pixel wide.
    for p in polys {
        budget::check_now();
        let n = p.uvs.len();
        for i in 0..n {
            let a = to_px(p.uvs[i], tile, wf, hf);
            let b = to_px(p.uvs[(i + 1) % n], tile, wf, hf);
            line(&mut fb, a, b);
        }
    }
    // OpenImageIO un-premultiplies before writing the PNG (in place, row by row).
    let mut px = fb.px;
    for row in px.chunks_mut(w * 4) {
        budget::check_n(u32::try_from(w).unwrap_or(u32::MAX));
        for p in row.chunks_exact_mut(4) {
            let a = p[3];
            if a == 0 {
                continue;
            }
            let af = f32::from(a) / 255.0;
            for c in &mut p[..3] {
                *c = unorm8(f32::from(*c) / 255.0 / af);
            }
        }
    }
    px
}

/// One `POLYLINE_UNIFORM_COLOR` segment: a quad `±1` pixel around `a`–`b`, alpha
/// `1 - |distance|`.
fn line(fb: &mut Fb, a: [f32; 2], b: [f32; 2]) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if len == 0.0 || !len.is_finite() {
        return;
    }
    let e = [d[0] / len, d[1] / len];
    let o = [-e[1], e[0]];
    let q = [
        [a[0] + o[0], a[1] + o[1]],
        [a[0] - o[0], a[1] - o[1]],
        [b[0] + o[0], b[1] + o[1]],
        [b[0] - o[0], b[1] - o[1]],
    ];
    let mut frags = Vec::new();
    for tri in [[q[0], q[1], q[2]], [q[1], q[3], q[2]]] {
        fb.triangle(tri, |x, y, cx, cy| {
            let s =
                (cx - f64::from(a[0])) * f64::from(o[0]) + (cy - f64::from(a[1])) * f64::from(o[1]);
            frags.push((x, y, s as f32));
        });
    }
    for (x, y, s) in frags {
        let alpha = (1.0 - s.abs()).clamp(0.0, 1.0);
        fb.blend(x, y, [0.0; 3], alpha);
    }
}

/// CRC-32 (ISO 3309, PNG chunks).
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *t = c;
    }
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c = table[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xffff_ffff
}

/// `IDAT`: the rows of `rgba` (filter `None`) deflated at zlib level 6, one cancellation
/// point of `--stmt-timeout` per row.
fn idat(rgba: &[u8], width: u32) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
    let stride = (width as usize * 4).max(1);
    for row in rgba.chunks(stride) {
        budget::check_n(width);
        // Writing into a `Vec` cannot fail.
        let _ = z.write_all(&[0]);
        let _ = z.write_all(row);
    }
    z.finish().unwrap_or_default()
}

/// An RGBA8 PNG of `rgba` (top row first).
pub(crate) fn encode(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let data = idat(rgba, width);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &data);
    chunk(b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
    }

    #[test]
    fn unit_square_face() {
        // A face over the whole tile: every pixel filled (0.8 grey at alpha 0.25 after the
        // un-premultiply), the border pixels half covered by the edges.
        let poly = Poly {
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            color: [0.8, 0.8, 0.8],
        };
        let img = draw((0, 0), &[poly], 8, 8, 0.25);
        let at = |x: usize, y: usize| &img[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4];
        assert_eq!(at(4, 4), &[203, 203, 203, 64]);
        // An edge pixel: the fill, then one line at distance 0.5 (alpha 0.5).
        assert!((159..=160).contains(&at(0, 4)[3]));
        let png = encode(&img, 8, 8);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The IDAT inflates back to the filtered rows.
        let len = u32::from_be_bytes(png[33..37].try_into().unwrap()) as usize;
        assert_eq!(&png[37..41], b"IDAT");
        let mut raw = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(&png[41..41 + len]),
            &mut raw,
        )
        .unwrap();
        assert_eq!(raw.len(), 8 * (8 * 4 + 1));
        assert_eq!(&raw[1..33], &img[..32]);
    }

    #[test]
    fn compressed() {
        // 512 × 512 of the default cube's layout: deflated, far below the 1 MiB of raw rows.
        let poly = Poly {
            uvs: vec![[0.375, 0.0], [0.625, 0.0], [0.625, 0.25], [0.375, 0.25]],
            color: [0.8, 0.8, 0.8],
        };
        let img = draw((0, 0), &[poly], 512, 512, 0.25);
        assert!(encode(&img, 512, 512).len() < 20_000);
    }

    #[test]
    fn buffer_size() {
        assert_eq!(buffer_bytes(32768, 32768), Some(4_294_967_296));
        assert_eq!(buffer_bytes(0, -3), Some(4));
        // Over the default `--max-elements` (50,000,000 × 32 bytes) in a scope.
        let _s = budget::scope(budget::Budget {
            timeout: None,
            max_elements: 50_000_000,
        });
        assert!(budget::reserve_buffer(BUFFER, buffer_bytes(32768, 32768)).is_err());
        assert!(budget::reserve_buffer(BUFFER, buffer_bytes(1024, 1024)).is_ok());
    }
}
