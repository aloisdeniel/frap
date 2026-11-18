// SPDX-License-Identifier: GPL-3.0-or-later
//! `IMB_scale(ibuf, size, IMBScaleFilter::Box, threaded=false)`, the filter of `Image.scale()`
//! (`BKE_image_scale`).
//!
//! Ported from blender/source/blender/imbuf/intern/scaling.cc (`imb_scale_box`,
//! `scale_down_x_func`, `scale_down_y_func`, `scale_up_x_func`, `scale_up_y_func`), Blender
//! 5.2.4, GPL-2.0-or-later. The horizontal pass runs first into a buffer of the same sample type
//! (bytes are rounded between the passes, as in Blender), then the vertical pass.

use super::imbuf::{CHANNELS, ImBuf, ImBufData};

type Px = [f32; 4];

#[inline]
fn add(a: Px, b: Px) -> Px {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}
#[inline]
fn sub(a: Px, b: Px) -> Px {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}
#[inline]
fn mul(a: Px, s: f32) -> Px {
    [a[0] * s, a[1] * s, a[2] * s, a[3] * s]
}
#[inline]
fn smul(s: f32, a: Px) -> Px {
    [s * a[0], s * a[1], s * a[2], s * a[3]]
}
#[inline]
fn neg(a: Px) -> Px {
    [-a[0], -a[1], -a[2], -a[3]]
}

/// A strided line of pixels: `n` pixels from `start`, `stride` apart.
#[derive(Clone, Copy)]
struct Line {
    start: usize,
    stride: usize,
}

impl Line {
    #[inline]
    fn at(&self, i: usize) -> usize {
        self.start + i * self.stride
    }
}

/// `scale_down_{x,y}_func` along one line: `n_src` source pixels to `n_dst`.
fn down_line(
    src: &[Px],
    sl: Line,
    n_src: usize,
    dst: &mut [Px],
    dl: Line,
    n_dst: usize,
    q: fn(Px) -> Px,
) {
    let addv = (n_src as f32 - 0.01) / n_dst as f32;
    let inv_add = 1.0 / addv;
    let last = n_src.saturating_sub(1);
    let mut p = 0usize;
    let mut sample = 0.0f32;
    let mut val: Px = [0.0; 4];
    for x in 0..n_dst {
        let mut nval = mul(neg(val), sample);
        sample += addv;
        while sample >= 1.0 {
            sample -= 1.0;
            nval = add(nval, src[sl.at(p.min(last))]);
            p += 1;
        }
        val = src[sl.at(p.min(last))];
        p += 1;
        let pix = mul(add(nval, smul(sample, val)), inv_add);
        dst[dl.at(x)] = q(pix);
        sample -= 1.0;
    }
}

/// `scale_up_{x,y}_func` along one line.
fn up_line(
    src: &[Px],
    sl: Line,
    n_src: usize,
    dst: &mut [Px],
    dl: Line,
    n_dst: usize,
    q: fn(Px) -> Px,
) {
    let addv = (n_src as f32 - 0.001) / n_dst as f32;
    // Special case: source is 1px wide (see #70356).
    if n_src == 1 {
        for x in 0..n_dst {
            dst[dl.at(x)] = src[sl.at(0)];
        }
        return;
    }
    let mut sample = -0.5 + addv * 0.5;
    let mut counter = 0usize;
    let mut p = 0usize;
    let mut val = src[sl.at(p)];
    let mut nval = src[sl.at(p + 1)];
    let mut diff = sub(nval, val);
    if n_src > 2 {
        p += 2;
        counter += 2;
    }
    for x in 0..n_dst {
        if sample >= 1.0 {
            sample -= 1.0;
            val = nval;
            nval = src[sl.at(p)];
            diff = sub(nval, val);
            if counter + 1 < n_src {
                p += 1;
                counter += 1;
            }
        }
        let pix = add(val, smul(sample.max(0.0), diff));
        dst[dl.at(x)] = q(pix);
        sample += addv;
    }
}

/// `store_pixel(float4, uchar4 *)`: `uchar4(math::round(pix))`.
fn quantize_byte(p: Px) -> Px {
    p.map(|v| f32::from(v.round() as i32 as u8))
}

fn keep(p: Px) -> Px {
    p
}

/// `imb_scale_box` on pixels (row-major, `w × h`).
fn scale_box(
    src: &[Px],
    (w, h): (usize, usize),
    (nw, nh): (usize, usize),
    q: fn(Px) -> Px,
) -> Vec<Px> {
    // Horizontal pass: w × h → nw × h.
    let mut tmp = vec![[0.0f32; 4]; nw * h];
    for y in 0..h {
        frap_math::budget::check_n(nw as u32);
        let sl = Line {
            start: y * w,
            stride: 1,
        };
        let dl = Line {
            start: y * nw,
            stride: 1,
        };
        if nw < w {
            down_line(src, sl, w, &mut tmp, dl, nw, q);
        } else {
            up_line(src, sl, w, &mut tmp, dl, nw, q);
        }
    }
    // Vertical pass: nw × h → nw × nh.
    let mut out = vec![[0.0f32; 4]; nw * nh];
    for x in 0..nw {
        frap_math::budget::check_n(nh as u32);
        let sl = Line {
            start: x,
            stride: nw,
        };
        let dl = Line {
            start: x,
            stride: nw,
        };
        if nh < h {
            down_line(&tmp, sl, h, &mut out, dl, nh, q);
        } else {
            up_line(&tmp, sl, h, &mut out, dl, nh, q);
        }
    }
    out
}

impl ImBuf {
    /// `IMB_scale(ibuf, (width, height), Box, false)`: false (nothing done) when the size is
    /// unchanged or not positive. Does not mark the buffer dirty (`BKE_image_scale` does).
    pub fn scale(&mut self, width: u32, height: u32) -> bool {
        scale(self, width, height)
    }
}

fn scale(buf: &mut ImBuf, width: u32, height: u32) -> bool {
    if width == 0 || height == 0 || (width, height) == (buf.x, buf.y) || buf.is_empty() {
        return false;
    }
    // The working set (`tmp` and `out` of `scale_box`, four floats per pixel, plus the
    // result) is checked against the statement's budget before anything is allocated.
    let work = |w: u32, h: u32| super::imbuf::buffer_bytes(w, h, true);
    let total = work(width, buf.y)
        .zip(work(width, height))
        .and_then(|(a, b)| a.checked_add(b)?.checked_add(b));
    frap_math::budget::check_buffer(super::imbuf::BUFFER, total);
    let (w, h) = (buf.x as usize, buf.y as usize);
    let (nw, nh) = (width as usize, height as usize);
    match &mut buf.data {
        ImBufData::Float(f) => {
            let px: Vec<Px> = f
                .chunks_exact(CHANNELS)
                .map(|c| [c[0], c[1], c[2], c[3]])
                .collect();
            let out = scale_box(&px, (w, h), (nw, nh), keep);
            *f = out.into_iter().flatten().collect();
        }
        ImBufData::Byte(b) => {
            let px: Vec<Px> = b
                .chunks_exact(CHANNELS)
                .map(|c| {
                    [
                        f32::from(c[0]),
                        f32::from(c[1]),
                        f32::from(c[2]),
                        f32::from(c[3]),
                    ]
                })
                .collect();
            let out = scale_box(&px, (w, h), (nw, nh), quantize_byte);
            *b = out.into_iter().flatten().map(|v| v as u8).collect();
        }
    }
    buf.x = width;
    buf.y = height;
    true
}
