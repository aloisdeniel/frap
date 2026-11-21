// SPDX-License-Identifier: GPL-3.0-or-later
//! JPEG, as Blender 5.2.4 writes it (`imbuf/intern/format_jpeg.cc`, GPL-2.0-or-later) with the
//! libjpeg-turbo 2.1.3 it links (IJG / BSD-3-Clause licences).
//!
//! Blender's `init_jpeg`: `jpeg_set_defaults` (YCbCr with 2x2 luma sampling, i.e. 4:2:0; the
//! standard quantisation tables scaled by `jpeg_quality_scaling`, limited to baseline; the
//! standard Huffman tables, no optimisation; a JFIF 1.01 `APP0` without density), then
//! `dct_method = JDCT_FLOAT` and `jpeg_set_quality(quality, true)` (`quality` ≤ 0 → 75, at most
//! 100). Grey images (`color_mode='BW'`) are written as one-component greyscale JPEGs. Before the
//! tables Blender writes its `NeoGeo` `APP1` marker (the quality) and one `COM` marker per stamp
//! metadata field (`Blender:<key>:<value>`; frap writes no stamp metadata).
//!
//! Ported from libjpeg-turbo 2.1.3: `jccolor.c` (`rgb_ycc_convert`, fixed-point tables),
//! `jcprepct.c` / `jcsample.c` (bottom and right edge replication, `h2v2_downsample` with the
//! alternating 1, 2 bias), `jcdctmgr.c` / `jfdctflt.c` (the AA&N float DCT and
//! `quantize_float`), `jccoefct.c` (dummy blocks at the right and bottom edges), `jchuff.c`
//! (`encode_one_block`, bit stuffing, padding with ones) and `jcmarker.c`. The aarch64 build
//! Blender's goldens come from runs the float DCT and quantisation in C (libjpeg-turbo has no
//! NEON version of them); colour conversion and downsampling are integer (the SIMD versions are
//! bit-exact) and the entropy coder is deterministic, so the files equal Blender's byte for byte
//! apart from the stamp `COM` markers (goldens in `encode/tests.rs`). The DCT is evaluated as the
//! C source reads, without fused multiply-adds: fusing the rotator's two `c * x + z` products, as
//! a compiler may, changes no coefficient of any golden.

use crate::codec::{DecodedImage, ImageError};

const DCTSIZE: usize = 8;
const DCTSIZE2: usize = 64;

/// `jpeg_natural_order`: zig-zag index → natural (row-major) index.
const NATURAL_ORDER: [usize; DCTSIZE2] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

const STD_LUMINANCE_QUANT_TBL: [u32; DCTSIZE2] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

const STD_CHROMINANCE_QUANT_TBL: [u32; DCTSIZE2] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

const BITS_DC_LUMINANCE: [u8; 17] = [0, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const VAL_DC_LUMINANCE: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const BITS_DC_CHROMINANCE: [u8; 17] = [0, 0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const VAL_DC_CHROMINANCE: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const BITS_AC_LUMINANCE: [u8; 17] = [0, 0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const VAL_AC_LUMINANCE: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];
const BITS_AC_CHROMINANCE: [u8; 17] = [0, 0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const VAL_AC_CHROMINANCE: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// `aanscalefactor` of `jcdctmgr.c` (float DCT divisors).
const AAN_SCALE_FACTOR: [f64; DCTSIZE] = [
    1.0,
    1.387039845,
    1.306562965,
    1.175875602,
    1.0,
    0.785694958,
    0.541196100,
    0.275899379,
];

/// Writes `image` (1 or 3 channels used: grey + alpha drops alpha, RGBA drops alpha) with
/// Blender's settings at `quality` (`ImBuf.foptions.quality`; Blender maps `image_settings`
/// qualities below 10 to 90 before this).
pub(crate) fn encode_jpeg(image: &DecodedImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    let img = match image.channels {
        2 => image.with_channels(1)?,
        4 => image.with_channels(3)?,
        _ => image.clone(),
    };
    let (w, h) = (img.width as usize, img.height as usize);
    if w > 65500 || h > 65500 {
        // JPEG_MAX_DIMENSION
        return Err(ImageError::Encode(format!(
            "JPEG: {w}x{h} is larger than 65500x65500"
        )));
    }
    let samples = img.to_u8();
    // `init_jpeg`: quality <= 0 → 75, at most 100.
    let jpeg_quality = if quality == 0 { 75 } else { quality.min(100) };
    let scale = quality_scaling(i32::from(jpeg_quality));
    let qtables = [
        quant_table(&STD_LUMINANCE_QUANT_TBL, scale),
        quant_table(&STD_CHROMINANCE_QUANT_TBL, scale),
    ];

    let grey = img.channels == 1;
    let (max_h, max_v) = if grey { (1, 1) } else { (2, 2) };
    let planes = colour_planes(&samples, w, h, grey);
    let specs: &[(u8, usize, usize, usize)] = if grey {
        &[(1, 1, 1, 0)]
    } else {
        &[(1, 2, 2, 0), (2, 1, 1, 1), (3, 1, 1, 1)]
    };
    let total_imcu_rows = h.div_ceil(max_v * DCTSIZE);
    let comps: Vec<Component> = specs
        .iter()
        .zip(&planes)
        .map(|(&(id, hs, vs, tbl), full)| {
            Component::new(id, hs, vs, tbl, full, w, h, max_h, max_v, total_imcu_rows)
        })
        .collect();

    let mut out = Vec::with_capacity(1024 + w * h / 4);
    write_headers(&mut out, quality, &qtables, &comps, w, h);

    let divisors = [float_divisors(&qtables[0]), float_divisors(&qtables[1])];
    let huff = [
        (
            HuffTable::derive(&BITS_DC_LUMINANCE, &VAL_DC_LUMINANCE),
            HuffTable::derive(&BITS_AC_LUMINANCE, &VAL_AC_LUMINANCE),
        ),
        (
            HuffTable::derive(&BITS_DC_CHROMINANCE, &VAL_DC_CHROMINANCE),
            HuffTable::derive(&BITS_AC_CHROMINANCE, &VAL_AC_CHROMINANCE),
        ),
    ];
    let mut bits = BitWriter::new(&mut out);
    let mut last_dc = vec![0i32; comps.len()];
    let interleaved = comps.len() > 1;
    let mcus_per_row = if interleaved {
        w.div_ceil(max_h * DCTSIZE)
    } else {
        comps[0].width_in_blocks
    };
    let mcu_rows = if interleaved {
        total_imcu_rows
    } else {
        comps[0].height_in_blocks
    };
    let mut blocks: Vec<[i16; DCTSIZE2]> = Vec::new();
    for mcu_row in 0..mcu_rows {
        // `--stmt-timeout` (ARCHITECTURE §12): a cancellation point per MCU row.
        frap_math::budget::check_now();
        for mcu_col in 0..mcus_per_row {
            for (ci, c) in comps.iter().enumerate() {
                // Non-interleaved scans have one-block MCUs.
                let (mcu_w, mcu_h) = if interleaved { (c.h, c.v) } else { (1, 1) };
                let last_col_width = if interleaved {
                    match c.width_in_blocks % c.h {
                        0 => c.h,
                        r => r,
                    }
                } else {
                    1
                };
                let last_row_height = if interleaved {
                    match c.height_in_blocks % c.v {
                        0 => c.v,
                        r => r,
                    }
                } else {
                    1
                };
                let blockcnt = if mcu_col + 1 < mcus_per_row {
                    mcu_w
                } else {
                    last_col_width
                };
                blocks.clear();
                for yindex in 0..mcu_h {
                    let real = mcu_row + 1 < mcu_rows || yindex < last_row_height;
                    let by = (mcu_row * mcu_h + yindex) * DCTSIZE;
                    if real {
                        for bi in 0..blockcnt {
                            let bx = (mcu_col * mcu_w + bi) * DCTSIZE;
                            blocks.push(forward_dct(c, bx, by, &divisors[c.tbl]));
                        }
                        // Dummy blocks at the right edge: AC zero, DC of the previous block.
                        for _ in blockcnt..mcu_w {
                            let dc = blocks.last().map_or(0, |b| b[0]);
                            let mut b = [0i16; DCTSIZE2];
                            b[0] = dc;
                            blocks.push(b);
                        }
                    } else {
                        // A dummy row at the bottom: every DC is the block before the row's.
                        let dc = blocks.last().map_or(0, |b| b[0]);
                        for _ in 0..mcu_w {
                            let mut b = [0i16; DCTSIZE2];
                            b[0] = dc;
                            blocks.push(b);
                        }
                    }
                }
                let (dc_tbl, ac_tbl) = &huff[c.tbl];
                for b in &blocks {
                    encode_one_block(&mut bits, b, &mut last_dc[ci], dc_tbl, ac_tbl);
                }
            }
        }
    }
    bits.flush();
    out.extend_from_slice(&[0xFF, 0xD9]);
    Ok(out)
}

/// `jpeg_quality_scaling`.
fn quality_scaling(quality: i32) -> i32 {
    let q = quality.clamp(1, 100);
    if q < 50 { 5000 / q } else { 200 - q * 2 }
}

/// `jpeg_add_quant_table` with `force_baseline` (natural order).
fn quant_table(basic: &[u32; DCTSIZE2], scale: i32) -> [u16; DCTSIZE2] {
    let mut t = [0u16; DCTSIZE2];
    for (q, &b) in t.iter_mut().zip(basic) {
        let temp = ((i64::from(b) * i64::from(scale) + 50) / 100).clamp(1, 255);
        *q = temp as u16;
    }
    t
}

/// The float divisors of `start_pass_fdctmgr` (`JDCT_FLOAT`).
#[allow(clippy::needless_range_loop)] // the C loop over (row, col)
fn float_divisors(q: &[u16; DCTSIZE2]) -> [f32; DCTSIZE2] {
    let mut d = [0f32; DCTSIZE2];
    for row in 0..DCTSIZE {
        for col in 0..DCTSIZE {
            let i = row * DCTSIZE + col;
            d[i] = (1.0 / (f64::from(q[i]) * AAN_SCALE_FACTOR[row] * AAN_SCALE_FACTOR[col] * 8.0))
                as f32;
        }
    }
    d
}

/// `rgb_ycc_convert` (or the grey copy): one full-resolution plane per component.
fn colour_planes(samples: &[u8], w: usize, h: usize, grey: bool) -> Vec<Vec<u8>> {
    if grey {
        return vec![samples.to_vec()];
    }
    const SCALEBITS: i32 = 16;
    const ONE_HALF: i64 = 1 << (SCALEBITS - 1);
    const CBCR_OFFSET: i64 = 128 << SCALEBITS;
    let fix = |x: f64| (x * f64::from(1i32 << SCALEBITS) + 0.5) as i64;
    let (fy_r, fy_g, fy_b) = (fix(0.29900), fix(0.58700), fix(0.11400));
    let (fcb_r, fcb_g, fhalf) = (fix(0.16874), fix(0.33126), fix(0.50000));
    let (fcr_g, fcr_b) = (fix(0.41869), fix(0.08131));
    let n = w * h;
    let (mut y, mut cb, mut cr) = (vec![0u8; n], vec![0u8; n], vec![0u8; n]);
    for (i, p) in samples.chunks_exact(3).enumerate() {
        let (r, g, b) = (i64::from(p[0]), i64::from(p[1]), i64::from(p[2]));
        y[i] = ((fy_r * r + fy_g * g + fy_b * b + ONE_HALF) >> SCALEBITS) as u8;
        // The tables add `CBCR_OFFSET + ONE_HALF - 1` to the 0.5 entries.
        cb[i] =
            ((-fcb_r * r - fcb_g * g + fhalf * b + CBCR_OFFSET + ONE_HALF - 1) >> SCALEBITS) as u8;
        cr[i] =
            ((fhalf * r - fcr_g * g - fcr_b * b + CBCR_OFFSET + ONE_HALF - 1) >> SCALEBITS) as u8;
    }
    vec![y, cb, cr]
}

/// A component after downsampling and edge expansion.
struct Component {
    id: u8,
    h: usize,
    v: usize,
    tbl: usize,
    width_in_blocks: usize,
    height_in_blocks: usize,
    /// `width_in_blocks * 8` columns, `total_iMCU_rows * v * 8` rows.
    plane: Vec<u8>,
    cols: usize,
}

impl Component {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: u8,
        h: usize,
        v: usize,
        tbl: usize,
        full: &[u8],
        w: usize,
        height: usize,
        max_h: usize,
        max_v: usize,
        total_imcu_rows: usize,
    ) -> Component {
        let width_in_blocks = (w * h).div_ceil(max_h * DCTSIZE);
        let height_in_blocks = (height * v).div_ceil(max_v * DCTSIZE);
        let cols = width_in_blocks * DCTSIZE;
        // `pre_process_data`: the last row group is completed by replicating the last row.
        let full_rows = height.div_ceil(max_v) * max_v;
        let full_row = |r: usize| &full[r.min(height - 1) * w..r.min(height - 1) * w + w];
        let mut plane = Vec::with_capacity(cols * total_imcu_rows * v * DCTSIZE);
        if h == max_h && v == max_v {
            // `fullsize_downsample` + `expand_right_edge`.
            for r in 0..full_rows {
                let src = full_row(r);
                plane.extend_from_slice(src);
                plane.resize(plane.len() + cols - w, src[w - 1]);
            }
        } else {
            // `h2v2_downsample` on rows expanded to `cols * 2`.
            let at = |row: &[u8], x: usize| u32::from(row[x.min(w - 1)]);
            for r in (0..full_rows).step_by(2) {
                let (in0, in1) = (full_row(r), full_row(r + 1));
                let mut bias = 1;
                for c in 0..cols {
                    let s =
                        at(in0, 2 * c) + at(in0, 2 * c + 1) + at(in1, 2 * c) + at(in1, 2 * c + 1);
                    plane.push(((s + bias) >> 2) as u8);
                    bias ^= 3;
                }
            }
        }
        // `expand_bottom_edge` up to whole iMCU rows.
        let rows = total_imcu_rows * v * DCTSIZE;
        let last = plane[plane.len() - cols..].to_vec();
        while plane.len() < rows * cols {
            plane.extend_from_slice(&last);
        }
        Component {
            id,
            h,
            v,
            tbl,
            width_in_blocks,
            height_in_blocks,
            plane,
            cols,
        }
    }
}

/// `forward_DCT_float`: sample conversion, `jpeg_fdct_float`, `quantize_float`.
fn forward_dct(c: &Component, x: usize, y: usize, divisors: &[f32; DCTSIZE2]) -> [i16; DCTSIZE2] {
    let mut ws = [0f32; DCTSIZE2];
    for r in 0..DCTSIZE {
        let row = &c.plane[(y + r) * c.cols + x..(y + r) * c.cols + x + DCTSIZE];
        for (k, &s) in row.iter().enumerate() {
            ws[r * DCTSIZE + k] = (i32::from(s) - 128) as f32;
        }
    }
    fdct_float(&mut ws);
    let mut out = [0i16; DCTSIZE2];
    for i in 0..DCTSIZE2 {
        let temp = ws[i] * divisors[i];
        out[i] = ((temp + 16384.5) as i32 - 16384) as i16;
    }
    out
}

/// `jpeg_fdct_float` (`jfdctflt.c`), the AA&N scaled float DCT, rows then columns. The
/// constants are the C source's literals, rounded to `f32` as `(FAST_FLOAT)` casts do.
#[allow(clippy::excessive_precision, clippy::approx_constant)]
fn fdct_float(data: &mut [f32; DCTSIZE2]) {
    let pass = |data: &mut [f32; DCTSIZE2], base: usize, step: usize| {
        let d = |k: usize| base + k * step;
        let tmp0 = data[d(0)] + data[d(7)];
        let tmp7 = data[d(0)] - data[d(7)];
        let tmp1 = data[d(1)] + data[d(6)];
        let tmp6 = data[d(1)] - data[d(6)];
        let tmp2 = data[d(2)] + data[d(5)];
        let tmp5 = data[d(2)] - data[d(5)];
        let tmp3 = data[d(3)] + data[d(4)];
        let tmp4 = data[d(3)] - data[d(4)];

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        data[d(0)] = tmp10 + tmp11;
        data[d(4)] = tmp10 - tmp11;

        let z1 = (tmp12 + tmp13) * 0.707_106_781_f32;
        data[d(2)] = tmp13 + z1;
        data[d(6)] = tmp13 - z1;

        let tmp10 = tmp4 + tmp5;
        let tmp11 = tmp5 + tmp6;
        let tmp12 = tmp6 + tmp7;

        let z5 = (tmp10 - tmp12) * 0.382_683_433_f32;
        let z2 = 0.541_196_100_f32 * tmp10 + z5;
        let z4 = 1.306_562_965_f32 * tmp12 + z5;
        let z3 = tmp11 * 0.707_106_781_f32;

        let z11 = tmp7 + z3;
        let z13 = tmp7 - z3;

        data[d(5)] = z13 + z2;
        data[d(3)] = z13 - z2;
        data[d(1)] = z11 + z4;
        data[d(7)] = z11 - z4;
    };
    for row in 0..DCTSIZE {
        pass(data, row * DCTSIZE, 1);
    }
    for col in 0..DCTSIZE {
        pass(data, col, DCTSIZE);
    }
}

/// `c_derived_tbl`: code and length per symbol.
struct HuffTable {
    code: [u16; 256],
    size: [u8; 256],
}

impl HuffTable {
    /// `jpeg_make_c_derived_tbl`.
    fn derive(bits: &[u8; 17], vals: &[u8]) -> HuffTable {
        let mut t = HuffTable {
            code: [0; 256],
            size: [0; 256],
        };
        let mut code: u16 = 0;
        let mut k = 0;
        for len in 1..=16u8 {
            for _ in 0..bits[len as usize] {
                let sym = vals[k] as usize;
                t.code[sym] = code;
                t.size[sym] = len;
                code += 1;
                k += 1;
            }
            code <<= 1;
        }
        t
    }
}

/// The entropy coder's bit buffer: MSB first, `0xFF` stuffed with `0x00`.
struct BitWriter<'a> {
    out: &'a mut Vec<u8>,
    acc: u32,
    nbits: u32,
}

impl<'a> BitWriter<'a> {
    fn new(out: &'a mut Vec<u8>) -> Self {
        BitWriter {
            out,
            acc: 0,
            nbits: 0,
        }
    }

    fn put(&mut self, code: u32, size: u32) {
        debug_assert!(size <= 16);
        self.acc = (self.acc << size) | (code & ((1 << size) - 1));
        self.nbits += size;
        while self.nbits >= 8 {
            self.nbits -= 8;
            let byte = (self.acc >> self.nbits) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
        }
        self.acc &= (1 << self.nbits) - 1;
    }

    /// `flush_bits`: pad the last byte with ones.
    fn flush(&mut self) {
        if self.nbits > 0 {
            let pad = 8 - self.nbits;
            self.put((1 << pad) - 1, pad);
        }
    }
}

/// Number of bits of a non-negative value.
fn nbits(v: i32) -> u32 {
    32 - (v as u32).leading_zeros()
}

/// `encode_one_block`.
fn encode_one_block(
    bits: &mut BitWriter<'_>,
    block: &[i16; DCTSIZE2],
    last_dc: &mut i32,
    dc: &HuffTable,
    ac: &HuffTable,
) {
    let mut temp = i32::from(block[0]) - *last_dc;
    *last_dc = i32::from(block[0]);
    let mut temp2 = temp;
    if temp < 0 {
        temp = -temp;
        temp2 -= 1;
    }
    let n = nbits(temp);
    bits.put(
        u32::from(dc.code[n as usize]),
        u32::from(dc.size[n as usize]),
    );
    if n > 0 {
        bits.put(temp2 as u32, n);
    }
    let mut r = 0;
    for &k in &NATURAL_ORDER[1..] {
        let mut temp = i32::from(block[k]);
        if temp == 0 {
            r += 1;
            continue;
        }
        while r > 15 {
            bits.put(u32::from(ac.code[0xF0]), u32::from(ac.size[0xF0]));
            r -= 16;
        }
        let mut temp2 = temp;
        if temp < 0 {
            temp = -temp;
            temp2 -= 1;
        }
        let n = nbits(temp);
        let sym = ((r << 4) + n) as usize;
        bits.put(u32::from(ac.code[sym]), u32::from(ac.size[sym]));
        bits.put(temp2 as u32, n);
        r = 0;
    }
    if r > 0 {
        bits.put(u32::from(ac.code[0]), u32::from(ac.size[0]));
    }
}

fn marker(out: &mut Vec<u8>, code: u8, payload: &[u8]) {
    out.extend_from_slice(&[0xFF, code]);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(payload);
}

/// SOI, JFIF `APP0`, Blender's `NeoGeo` `APP1`, then `write_frame_header` (DQT per table, SOF0)
/// and `write_scan_header` (DHT per table in first use order, SOS).
fn write_headers(
    out: &mut Vec<u8>,
    quality: u8,
    qtables: &[[u16; DCTSIZE2]; 2],
    comps: &[Component],
    w: usize,
    h: usize,
) {
    out.extend_from_slice(&[0xFF, 0xD8]);
    // JFIF 1.01, density unit 0, density 1:1, no thumbnail.
    marker(
        out,
        0xE0,
        &[b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0],
    );
    // `NeoGeo` + `NeoGeo_Word` {pad1, pad2, pad3, quality}.
    marker(
        out,
        0xE1,
        &[b'N', b'e', b'o', b'G', b'e', b'o', 0, 0, 0, quality],
    );
    let mut sent = [false; 2];
    for c in comps {
        if !sent[c.tbl] {
            sent[c.tbl] = true;
            let mut p = vec![c.tbl as u8];
            p.extend(NATURAL_ORDER.iter().map(|&k| qtables[c.tbl][k] as u8));
            marker(out, 0xDB, &p);
        }
    }
    let mut sof = vec![8];
    sof.extend_from_slice(&(h as u16).to_be_bytes());
    sof.extend_from_slice(&(w as u16).to_be_bytes());
    sof.push(comps.len() as u8);
    for c in comps {
        sof.extend_from_slice(&[c.id, ((c.h << 4) + c.v) as u8, c.tbl as u8]);
    }
    marker(out, 0xC0, &sof);
    let tables: [[(&[u8; 17], &[u8]); 2]; 2] = [
        [
            (&BITS_DC_LUMINANCE, &VAL_DC_LUMINANCE),
            (&BITS_AC_LUMINANCE, &VAL_AC_LUMINANCE),
        ],
        [
            (&BITS_DC_CHROMINANCE, &VAL_DC_CHROMINANCE),
            (&BITS_AC_CHROMINANCE, &VAL_AC_CHROMINANCE),
        ],
    ];
    let mut sent = [false; 2];
    for c in comps {
        if sent[c.tbl] {
            continue;
        }
        sent[c.tbl] = true;
        for (class, (bits, vals)) in tables[c.tbl].iter().enumerate() {
            let mut p = vec![((class << 4) + c.tbl) as u8];
            p.extend_from_slice(&bits[1..]);
            p.extend_from_slice(vals);
            marker(out, 0xC4, &p);
        }
    }
    let mut sos = vec![comps.len() as u8];
    for c in comps {
        sos.extend_from_slice(&[c.id, ((c.tbl << 4) + c.tbl) as u8]);
    }
    sos.extend_from_slice(&[0, 63, 0]);
    marker(out, 0xDA, &sos);
}
