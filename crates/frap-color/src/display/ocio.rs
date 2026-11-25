// SPDX-License-Identifier: GPL-3.0-or-later
//! OCIO op pipelines: the interpreter of `assets/luts/*.ops`. WP 5.3.
//!
//! `tools/blender/bake_view_luts.py` asks Blender 5.2.4's bundled OpenColorIO for the processor
//! of every display × view × look Blender builds for a render (`create_ocio_display_transform`,
//! optimised with `OPTIMIZATION_LOSSLESS`) and writes its op list, one `[display|view|look]`
//! section per combination and one op per line. This module parses those sections and applies
//! the ops with ports of OCIO 2.5's CPU renderers (`src/OpenColorIO/ops/*/…OpCPU.cpp`,
//! BSD-3-Clause, Copyright Contributors to the OpenColorIO Project); the colour-grading ops live
//! in [`crate::looks`] and the ACES fixed functions in [`crate::aces`].
//!
//! Op lines (numbers are the f64 values OCIO holds; renderers compute in f32 as OCIO's do):
//!
//! ```text
//! matrix m00 m01 m02 m10 m11 m12 m20 m21 m22 o0 o1 o2     MatrixOffsetOp
//! log <base> fwd|inv                                       LogOp (log / anti-log)
//! lut1d <data> fwd|inv                                     Lut1DOp, linear (inverse: exact)
//! lut3d <data>                                             Lut3DOp, tetrahedral
//! exponent g0 g1 g2 clamp|mirror|pass_thru fwd|inv         GammaOp, basic styles
//! moncurve g0 g1 g2 o0 o1 o2 linear|mirror fwd|inv         GammaOp, moncurve styles
//! range minIn maxIn minOut maxOut                          RangeOp ("-" = unset)
//! cdl s0 s1 s2 o0 o1 o2 p0 p1 p2 sat clamp|noclamp fwd     CDLOp
//! primary_log … fwd                                        GradingPrimaryOp, log style
//! tone_log … fwd                                           GradingToneOp, log style
//! curve_log … fwd                                          GradingRGBCurveOp, log style
//! fixed <style> fwd [params…]                              FixedFunctionOp (ACES)
//! ```
//!
//! LUT data (`<data>`) is resolved by name across the families' `DATA` lists: `.cube` text for
//! 3D LUTs (red fastest), `.lut1d` text for 1D LUTs (`LUT_1D_SIZE n`, `CHANNELS 1|3`, one entry
//! per line).

use std::sync::OnceLock;

use crate::lut::{CubeFile, Lut3d};
use crate::{aces, looks};

/// One baked op file of a view family: its text (`include_str!` of `assets/luts/<family>.ops`).
pub(crate) type OpsFile = &'static str;

/// One baked LUT data file: `(name, deflate-packed text)` (`src/generated/`, decision log
/// BIN-1; parsed through [`crate::lut::unpack`] on first use).
pub(crate) type DataFile = (&'static str, &'static [u8]);

/// Every `.ops` file, from the family modules.
fn ops_files() -> [OpsFile; 6] {
    [
        super::STANDARD_PIPELINES,
        crate::aces::PIPELINES,
        crate::pbr_neutral::PIPELINES,
        crate::agx::PIPELINES,
        crate::filmic::PIPELINES,
        crate::false_color::PIPELINES,
    ]
}

/// Every LUT data file, from the family modules.
fn data_files() -> impl Iterator<Item = &'static DataFile> {
    crate::agx::DATA
        .iter()
        .chain(crate::filmic::DATA)
        .chain(crate::pbr_neutral::DATA)
        .chain(crate::false_color::DATA)
        .chain(crate::aces::DATA)
}

// --- LUT data ------------------------------------------------------------------------------------

/// A 1D LUT over `[0, 1]` (OCIO `Lut1DOpData` without half domain), one table per channel.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut1d {
    /// `[r, g, b]` tables of equal length (identical for single-channel files).
    pub tables: [Vec<f32>; 3],
}

impl Lut1d {
    /// Parses `.lut1d` text: `TITLE`/`#` lines, `LUT_1D_SIZE n`, `CHANNELS 1|3`, `n` entries.
    pub fn parse(text: &str) -> Result<Lut1d, String> {
        let mut size: Option<usize> = None;
        let mut channels = 0usize;
        let mut values: Vec<[f32; 3]> = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("TITLE") {
                continue;
            }
            let bad = |what: &str| format!(".lut1d line {}: {what}", i + 1);
            if let Some(rest) = line.strip_prefix("LUT_1D_SIZE") {
                let n: usize = rest.trim().parse().map_err(|_| bad("bad LUT_1D_SIZE"))?;
                if !(2..=1 << 20).contains(&n) {
                    return Err(bad("LUT_1D_SIZE out of range"));
                }
                size = Some(n);
                values.reserve(n);
                continue;
            }
            if let Some(rest) = line.strip_prefix("CHANNELS") {
                channels = rest.trim().parse().map_err(|_| bad("bad CHANNELS"))?;
                if channels != 1 && channels != 3 {
                    return Err(bad("CHANNELS must be 1 or 3"));
                }
                continue;
            }
            if channels == 0 {
                return Err(bad("data before CHANNELS"));
            }
            let mut v = [0.0f32; 3];
            let mut it = line.split_whitespace();
            for c in v.iter_mut().take(channels) {
                *c = it
                    .next()
                    .and_then(|t| t.parse().ok())
                    .ok_or_else(|| bad("bad number"))?;
            }
            if it.next().is_some() {
                return Err(bad("too many numbers"));
            }
            if channels == 1 {
                v = [v[0]; 3];
            }
            values.push(v);
        }
        let size = size.ok_or("missing LUT_1D_SIZE")?;
        if values.len() != size {
            return Err(format!("expected {size} entries, found {}", values.len()));
        }
        Ok(Lut1d {
            tables: [0, 1, 2].map(|c| values.iter().map(|v| v[c]).collect()),
        })
    }
}

/// OCIO's `lerpf(a, b, z)`.
#[inline]
fn lerpf(a: f32, b: f32, z: f32) -> f32 {
    (b - a) * z + a
}

// --- OCIO's fast math (`SSE.h`) -------------------------------------------------------------
//
// Blender's CPU processors are OCIO's default ones (`getDefaultCPUProcessor`), optimised with
// `OPTIMIZATION_DEFAULT`, which includes `OPTIMIZATION_FAST_LOG_EXP_POW`: the gamma, CDL and
// log renderers then use `ssePower`/`sseLog2`/`sseExp2`, Chebyshev approximations good to ~15
// bits of mantissa. Scalar ports, bit-for-bit (they matter: False Color's banded 1D LUT turns a
// 1e-5 difference into a different band).

// `(float)` of the double literals, as `SSE.h` writes them.
#[allow(clippy::excessive_precision)]
const PNLOG: [f32; 6] = [
    -2.800_364_054_395_965_731_506_f64 as f32,
    5.091_710_879_305_474_367_557_f64 as f32,
    -3.550_793_018_041_176_193_407_f64 as f32,
    1.631_148_826_119_436_277_100_f64 as f32,
    -4.165_637_071_209_677_112_635e-1_f64 as f32,
    4.487_361_286_440_374_006_195e-2_f64 as f32,
];
#[allow(clippy::excessive_precision)]
const PNEXP: [f32; 5] = [
    1.000_002_593_370_603_213_644_f64 as f32,
    6.930_038_344_665_415_134_202e-1_f64 as f32,
    2.414_427_569_091_865_207_710e-1_f64 as f32,
    5.201_146_058_412_685_018_921e-2_f64 as f32,
    1.353_416_792_833_547_468_620e-2_f64 as f32,
];

/// `sseLog2`.
#[inline]
pub(crate) fn sse_log2(x: f32) -> f32 {
    const EXP_MASK: u32 = 0x7F80_0000;
    let bits = x.to_bits();
    let mantissa = f32::from_bits((bits & !EXP_MASK) | 1.0f32.to_bits());
    let m = mantissa;
    let poly =
        ((((PNLOG[5] * m + PNLOG[4]) * m + PNLOG[3]) * m + PNLOG[2]) * m + PNLOG[1]) * m + PNLOG[0];
    let exponent = ((bits & EXP_MASK) >> 23) as i32 - 127;
    poly + exponent as f32
}

/// `_mm_cvttps_epi32`: truncation, "integer indefinite" (`i32::MIN`) out of range or NaN.
#[inline]
fn cvtt(x: f32) -> i32 {
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        i32::MIN
    } else {
        x as i32
    }
}

/// `sseExp2`.
#[inline]
pub(crate) fn sse_exp2(x: f32) -> f32 {
    // NOT(0 <= x) adds -1 (negatives and NaN).
    let floor_x = cvtt(x).wrapping_add(if 0.0 <= x { 0 } else { -1 });
    let zf = f32::from_bits((floor_x.wrapping_add(127) as u32).wrapping_shl(23));
    let fraction = x - floor_x as f32;
    let f = fraction;
    let mexp = (((PNEXP[4] * f + PNEXP[3]) * f + PNEXP[2]) * f + PNEXP[1]) * f + PNEXP[0];
    let mut e = zf * mexp;
    if x < -126.0 {
        e = 0.0;
    }
    if x >= 128.0 {
        e = f32::INFINITY;
    }
    e
}

/// `ssePower`: `exp2(exp * log2(x))`, 0 for `x <= 0` (and NaN).
#[inline]
pub(crate) fn sse_power(x: f32, exp: f32) -> f32 {
    let v = sse_exp2(exp * sse_log2(x));
    if x > 0.0 { v } else { 0.0 }
}

/// `Lut1DRenderer::apply` (f32 in/out): linear interpolation, input clamped to the domain,
/// NaN → 0.
#[inline]
fn lut1d_forward(table: &[f32], x: f32) -> f32 {
    let dim_minus_one = (table.len() - 1) as f32;
    let idx = dim_minus_one * x;
    // std::min(std::max(0.f, idx), dimMinusOne): NaN becomes 0.
    let idx = if idx > 0.0 {
        idx.min(dim_minus_one)
    } else {
        0.0
    };
    let low = libm::floorf(idx) as usize;
    let high = libm::ceilf(idx) as usize;
    let delta = high as f32 - idx;
    lerpf(table[high], table[low], delta)
}

/// The exact inverse of a 1D LUT channel (`InvLut1DRenderer`, non-half domain), prepared as
/// `Lut1DOpData::initializeFromForward` does: reversals flattened, decreasing LUTs negated,
/// the flat ends excluded from the search.
#[derive(Clone, Debug, PartialEq)]
struct InvChannel {
    values: Vec<f32>,
    start: usize,
    end: usize,
    flip: f32,
}

impl InvChannel {
    fn new(table: &[f32]) -> InvChannel {
        let len = table.len();
        let mut values = table.to_vec();
        // Overall direction from the first and last entries (flat LUTs count as decreasing).
        let increasing = values[0] < values[len - 1];
        // Flatten reversals.
        let mut prev = values[0];
        for v in values.iter_mut().skip(1) {
            if increasing != (*v > prev) {
                *v = prev;
            } else {
                prev = *v;
            }
        }
        // Effective domain without the flat spots at either end.
        let mut end = len - 1;
        let end_value = values[end];
        while end > 0 && values[end - 1] == end_value {
            end -= 1;
        }
        let mut start = 0;
        let start_value = values[0];
        while start < end && values[start + 1] == start_value {
            start += 1;
        }
        if !increasing {
            for v in &mut values {
                *v = -*v;
            }
        }
        InvChannel {
            values,
            start,
            end,
            flip: if increasing { 1.0 } else { -1.0 },
        }
    }

    /// `FindLutInv` with `scale = 1 / (dim - 1)`.
    #[inline]
    fn apply(&self, val: f32) -> f32 {
        let lut = &self.values;
        let (first, last) = (lut[self.start], lut[self.end]);
        // std::min(std::max(val * flipSign, *start), *end)
        let x = val * self.flip;
        let cv = if x < first { first } else { x };
        let cv = if last < cv { last } else { cv };
        // std::lower_bound over [start, end).
        let mut low = self.start + lut[self.start..self.end].partition_point(|v| *v < cv);
        if low > self.start {
            low -= 1;
        }
        let high = if low < self.end { low + 1 } else { low };
        let mut delta = 0.0;
        if lut[high] > lut[low] {
            delta = (cv - lut[low]) / (lut[high] - lut[low]);
        }
        let scale = 1.0 / (lut.len() - 1) as f32;
        (low as f32 + delta) * scale
    }
}

#[derive(Debug)]
enum LutData {
    One(Lut1d),
    Three(Lut3d),
}

struct DataEntry {
    name: &'static str,
    packed: &'static [u8],
    parsed: OnceLock<Result<LutData, String>>,
}

fn data(name: &str) -> Result<&'static LutData, String> {
    static ENTRIES: OnceLock<Vec<DataEntry>> = OnceLock::new();
    let entries = ENTRIES.get_or_init(|| {
        data_files()
            .map(|(name, packed)| DataEntry {
                name,
                packed,
                parsed: OnceLock::new(),
            })
            .collect()
    });
    let entry = entries
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("no LUT data named '{name}'"))?;
    entry
        .parsed
        .get_or_init(|| {
            let text = crate::lut::unpack(entry.packed);
            if text.contains("LUT_3D_SIZE") {
                CubeFile::parse(&text)
                    .map(|c| LutData::Three(c.lut))
                    .map_err(|e| e.to_string())
            } else {
                Lut1d::parse(&text).map(LutData::One)
            }
        })
        .as_ref()
        .map_err(|e| format!("LUT data '{name}': {e}"))
}

// --- ops -----------------------------------------------------------------------------------------

/// `GammaOpData` basic negative styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NegativeStyle {
    Clamp,
    Mirror,
    PassThru,
}

/// Moncurve renderer parameters of one channel (`GammaOpUtils.cpp`, `RendererParams`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Moncurve {
    gamma: f32,
    offset: f32,
    break_pnt: f32,
    slope: f32,
    scale: f32,
}

impl Moncurve {
    /// `ComputeParamsFwd` / `ComputeParamsRev` (computed in f64, stored as f32).
    fn new(gamma: f64, offset: f64, forward: bool) -> Moncurve {
        const EPS: f64 = 1e-6;
        let g = gamma.max(1.0 + EPS);
        let o = offset.max(EPS);
        if forward {
            let a = (g - 1.0) / o;
            let b = o * g / ((g - 1.0) * (1.0 + o));
            Moncurve {
                gamma: g as f32,
                offset: (o / (1.0 + o)) as f32,
                break_pnt: (o / (g - 1.0)) as f32,
                slope: (a * libm::pow(b, g)) as f32,
                scale: (1.0 / (1.0 + o)) as f32,
            }
        } else {
            let a = (g - 1.0) / o;
            let b = (1.0 + o) / g;
            Moncurve {
                gamma: (1.0 / g) as f32,
                offset: o as f32,
                break_pnt: libm::pow(o * g / ((g - 1.0) * (1.0 + o)), g) as f32,
                slope: (libm::pow(a, g - 1.0) * libm::pow(b, g)) as f32,
                scale: (1.0 + o) as f32,
            }
        }
    }

    /// `GammaMoncurveOpCPUFwdSSE` (encoded → linear).
    #[inline]
    fn forward(&self, x: f32) -> f32 {
        if x > self.break_pnt {
            sse_power(x * self.scale + self.offset, self.gamma)
        } else {
            x * self.slope
        }
    }

    /// `GammaMoncurveOpCPURevSSE` (linear → encoded).
    #[inline]
    fn reverse(&self, x: f32) -> f32 {
        if x > self.break_pnt {
            sse_power(x, self.gamma) * self.scale - self.offset
        } else {
            x * self.slope
        }
    }
}

/// One op of a pipeline.
#[derive(Debug)]
enum Op {
    /// `out = m · in + offset`.
    Matrix {
        m: [f32; 9],
        offset: [f32; 3],
    },
    /// `log2(max(x, FLT_MIN)) * scale`.
    Log {
        scale: f32,
    },
    /// `exp2(x * log2(base))`.
    AntiLog {
        log2_base: f32,
    },
    Lut1dForward(&'static Lut1d),
    Lut1dInverse(Box<[InvChannel; 3]>),
    Lut3d(&'static Lut3d),
    Exponent {
        power: [f32; 3],
        style: NegativeStyle,
    },
    Moncurve {
        params: [Moncurve; 3],
        forward: bool,
        mirror: bool,
    },
    Range {
        scale: f32,
        offset: f32,
        lower: Option<f32>,
        upper: Option<f32>,
    },
    Cdl(looks::Cdl),
    PrimaryLog(looks::GradingPrimaryLog),
    ToneLog(Box<looks::GradingToneLog>),
    CurveLog(Box<aces::RgbCurveLog>),
    Glow10,
    RedMod10,
    DarkToDim10,
    GamutComp13(aces::GamutComp13),
    OutputTransform20(Box<aces::OutputTransform20>),
}

/// `std::max(a, b)` / `std::min(a, b)` as C++ evaluates them (NaN handling included).
#[inline]
fn cpp_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

#[inline]
fn cpp_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

impl Op {
    #[inline]
    fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        match self {
            Op::Matrix { m, offset } => [
                p[0] * m[0] + p[1] * m[1] + p[2] * m[2] + offset[0],
                p[0] * m[3] + p[1] * m[4] + p[2] * m[5] + offset[1],
                p[0] * m[6] + p[1] * m[7] + p[2] * m[8] + offset[2],
            ],
            Op::Log { scale } => p.map(|v| sse_log2(cpp_max(f32::MIN_POSITIVE, v)) * scale),
            Op::AntiLog { log2_base } => p.map(|v| sse_exp2(v * log2_base)),
            Op::Lut1dForward(lut) => [
                lut1d_forward(&lut.tables[0], p[0]),
                lut1d_forward(&lut.tables[1], p[1]),
                lut1d_forward(&lut.tables[2], p[2]),
            ],
            Op::Lut1dInverse(inv) => [inv[0].apply(p[0]), inv[1].apply(p[1]), inv[2].apply(p[2])],
            Op::Lut3d(lut) => lut.tetrahedral(p),
            Op::Exponent { power, style } => [0, 1, 2].map(|c| {
                let (v, g) = (p[c], power[c]);
                match style {
                    NegativeStyle::Clamp => sse_power(v, g),
                    NegativeStyle::Mirror => libm::copysignf(sse_power(v.abs(), g), v),
                    NegativeStyle::PassThru => {
                        if v > 0.0 {
                            sse_power(v, g)
                        } else {
                            v
                        }
                    }
                }
            }),
            Op::Moncurve {
                params,
                forward,
                mirror,
            } => [0, 1, 2].map(|c| {
                let m = &params[c];
                let f = |x: f32| if *forward { m.forward(x) } else { m.reverse(x) };
                if *mirror {
                    libm::copysignf(1.0, p[c]) * f(p[c].abs())
                } else {
                    f(p[c])
                }
            }),
            Op::Range {
                scale,
                offset,
                lower,
                upper,
            } => p.map(|v| {
                let t = v * scale + offset;
                match (lower, upper) {
                    // OCIO `Clamp(t, lo, hi)`: NaNs become the lower bound.
                    (Some(lo), Some(hi)) => {
                        if t > *hi {
                            *hi
                        } else if t >= *lo {
                            t
                        } else {
                            *lo
                        }
                    }
                    (Some(lo), None) => cpp_max(*lo, t),
                    (None, Some(hi)) => cpp_min(*hi, t),
                    (None, None) => t,
                }
            }),
            Op::Cdl(c) => c.apply(p),
            Op::PrimaryLog(g) => g.apply(p),
            Op::ToneLog(g) => g.apply(p),
            Op::CurveLog(c) => c.apply(p),
            Op::Glow10 => aces::glow_10(p),
            Op::RedMod10 => aces::red_mod_10(p),
            Op::DarkToDim10 => aces::dark_to_dim_10(p),
            Op::GamutComp13(g) => g.apply(p),
            Op::OutputTransform20(t) => t.apply(p),
        }
    }
}

/// Parses the numbers of an op line.
pub(crate) fn numbers(tokens: &[&str]) -> Result<Vec<f64>, String> {
    tokens
        .iter()
        .map(|t| t.parse::<f64>().map_err(|_| format!("bad number {t:?}")))
        .collect()
}

/// Exactly `n` numbers.
pub(crate) fn numbers_n(tokens: &[&str], n: usize) -> Result<Vec<f64>, String> {
    if tokens.len() != n {
        return Err(format!("expected {n} numbers, found {}", tokens.len()));
    }
    numbers(tokens)
}

fn direction(token: Option<&&str>) -> Result<bool, String> {
    match token.copied() {
        Some("fwd") => Ok(true),
        Some("inv") => Ok(false),
        other => Err(format!("expected fwd|inv, found {other:?}")),
    }
}

fn parse_op(line: &str) -> Result<Op, String> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let (name, args) = tokens.split_first().ok_or("empty op")?;
    let forward_only = |dir: Option<&&str>| -> Result<(), String> {
        if direction(dir)? {
            Ok(())
        } else {
            Err(format!("{name}: only the forward direction is implemented"))
        }
    };
    Ok(match *name {
        "matrix" => {
            let v = numbers_n(args, 12)?;
            let m = [0, 1, 2, 3, 4, 5, 6, 7, 8].map(|i| v[i] as f32);
            Op::Matrix {
                m,
                offset: [v[9] as f32, v[10] as f32, v[11] as f32],
            }
        }
        "log" => {
            let base = numbers_n(&args[..1.min(args.len())], 1)?[0];
            if !(base > 0.0 && base != 1.0) {
                return Err(format!("log: bad base {base}"));
            }
            let log2_base = libm::log2(base);
            if direction(args.get(1))? {
                // LOG10_2 for base 10, as OCIO's LogRenderer.
                Op::Log {
                    scale: (1.0 / log2_base) as f32,
                }
            } else {
                Op::AntiLog {
                    log2_base: log2_base as f32,
                }
            }
        }
        "lut1d" => {
            let name = args.first().ok_or("lut1d: missing data name")?;
            let LutData::One(lut) = data(name)? else {
                return Err(format!("lut1d: '{name}' is not a 1D LUT"));
            };
            if direction(args.get(1))? {
                Op::Lut1dForward(lut)
            } else {
                Op::Lut1dInverse(Box::new([0, 1, 2].map(|c| InvChannel::new(&lut.tables[c]))))
            }
        }
        "lut3d" => {
            let name = args.first().ok_or("lut3d: missing data name")?;
            let LutData::Three(lut) = data(name)? else {
                return Err(format!("lut3d: '{name}' is not a 3D LUT"));
            };
            Op::Lut3d(lut)
        }
        "exponent" => {
            if args.len() != 5 {
                return Err("exponent: expected 3 values, a style and a direction".into());
            }
            let v = numbers_n(&args[..3], 3)?;
            let style = match args[3] {
                "clamp" => NegativeStyle::Clamp,
                "mirror" => NegativeStyle::Mirror,
                "pass_thru" => NegativeStyle::PassThru,
                s => return Err(format!("exponent: unknown style {s}")),
            };
            let forward = direction(args.get(4))?;
            Op::Exponent {
                power: [0, 1, 2].map(|c| if forward { v[c] } else { 1.0 / v[c] } as f32),
                style,
            }
        }
        "moncurve" => {
            if args.len() != 8 {
                return Err("moncurve: expected 6 values, a style and a direction".into());
            }
            let v = numbers_n(&args[..6], 6)?;
            let mirror = match args[6] {
                "linear" => false,
                "mirror" => true,
                s => return Err(format!("moncurve: unknown style {s}")),
            };
            let forward = direction(args.get(7))?;
            Op::Moncurve {
                params: [0, 1, 2].map(|c| Moncurve::new(v[c], v[c + 3], forward)),
                forward,
                mirror,
            }
        }
        "range" => {
            if args.len() != 4 {
                return Err("range: expected 4 values".into());
            }
            let v: Vec<Option<f64>> = args
                .iter()
                .map(|t| {
                    if *t == "-" {
                        Ok(None)
                    } else {
                        t.parse::<f64>()
                            .map(Some)
                            .map_err(|_| format!("bad number {t:?}"))
                    }
                })
                .collect::<Result<_, _>>()?;
            // `RangeOpData::fillScaleOffset`.
            let (scale, offset) = match (v[0], v[1], v[2], v[3]) {
                (Some(min_in), Some(max_in), Some(min_out), Some(max_out)) => {
                    let denom = max_in - min_in;
                    if denom.abs() < 1e-6 {
                        return Err("range: maxInValue is too close to minInValue".into());
                    }
                    let scale = (max_out - min_out) / denom;
                    (scale, min_out - scale * min_in)
                }
                _ => (1.0, 0.0),
            };
            if v[0].is_some() != v[2].is_some() || v[1].is_some() != v[3].is_some() {
                return Err("range: in and out bounds must be set together".into());
            }
            Op::Range {
                scale: scale as f32,
                offset: offset as f32,
                lower: v[2].map(|x| x as f32),
                upper: v[3].map(|x| x as f32),
            }
        }
        "cdl" => {
            let (dir, rest) = args.split_last().ok_or("cdl: empty")?;
            forward_only(Some(dir))?;
            let (style, values) = rest.split_last().ok_or("cdl: empty")?;
            Op::Cdl(looks::Cdl::new(&numbers_n(values, 10)?, *style == "clamp")?)
        }
        "primary_log" => {
            let (dir, values) = args.split_last().ok_or("primary_log: empty")?;
            forward_only(Some(dir))?;
            Op::PrimaryLog(looks::GradingPrimaryLog::new(&numbers_n(values, 18)?))
        }
        "tone_log" => {
            let (dir, values) = args.split_last().ok_or("tone_log: empty")?;
            forward_only(Some(dir))?;
            Op::ToneLog(Box::new(looks::GradingToneLog::new(&numbers_n(
                values, 31,
            )?)))
        }
        "curve_log" => {
            let (dir, values) = args.split_last().ok_or("curve_log: empty")?;
            forward_only(Some(dir))?;
            Op::CurveLog(Box::new(aces::RgbCurveLog::new(&numbers(values)?)?))
        }
        "fixed" => {
            let style = args.first().ok_or("fixed: missing style")?;
            forward_only(args.get(1))?;
            let params = numbers(args.get(2..).unwrap_or(&[]))?;
            let no_params = |op: Op| {
                if params.is_empty() {
                    Ok(op)
                } else {
                    Err(format!("fixed {style}: takes no parameters"))
                }
            };
            match *style {
                "aces_glow_10" => no_params(Op::Glow10)?,
                "aces_red_mod_10" => no_params(Op::RedMod10)?,
                "aces_dark_to_dim_10" => no_params(Op::DarkToDim10)?,
                "aces_gamut_comp_13" => Op::GamutComp13(aces::GamutComp13::new(&params)?),
                "aces_output_transform_20" => {
                    Op::OutputTransform20(Box::new(aces::OutputTransform20::new(&params)?))
                }
                s => return Err(format!("fixed: unknown style {s}")),
            }
        }
        other => return Err(format!("unknown op '{other}'")),
    })
}

/// A parsed op pipeline: scene-linear Rec.709 → display-encoded RGB.
#[derive(Debug)]
pub(crate) struct Pipeline {
    ops: Vec<Op>,
}

impl Pipeline {
    /// Parses the op lines of one section.
    fn parse(body: &str) -> Result<Pipeline, String> {
        let mut ops = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            ops.push(parse_op(line).map_err(|e| format!("{e} (in `{line:.60}`)"))?);
        }
        Ok(Pipeline { ops })
    }

    /// Number of ops (0 for Raw).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.ops.len()
    }

    /// Applies every op in order.
    #[inline]
    pub(crate) fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        self.ops.iter().fold(rgb, |p, op| op.apply(p))
    }
}

struct Section {
    display: &'static str,
    view: &'static str,
    look: &'static str,
    body: &'static str,
    parsed: OnceLock<Result<Pipeline, String>>,
}

fn sections() -> &'static [Section] {
    static SECTIONS: OnceLock<Vec<Section>> = OnceLock::new();
    SECTIONS.get_or_init(|| {
        let mut out = Vec::new();
        for file in ops_files() {
            // Section headers start a line with `[`; the body runs to the next header.
            let mut rest = file;
            while let Some(at) = rest.find("\n[") {
                let after = &rest[at + 2..];
                let close = after
                    .find(']')
                    .expect("assets/luts/*.ops: unclosed section header");
                let header = &after[..close];
                let body_start = &after[close + 1..];
                let body_len = body_start.find("\n[").unwrap_or(body_start.len());
                let mut parts = header.split('|');
                let (Some(display), Some(view), Some(look), None) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                else {
                    panic!("assets/luts/*.ops: bad section header [{header}]");
                };
                out.push(Section {
                    display,
                    view,
                    look,
                    body: &body_start[..body_len],
                    parsed: OnceLock::new(),
                });
                rest = &body_start[body_len..];
            }
        }
        out
    })
}

/// The baked pipeline for `(display, view, look)` (`look` already filtered by
/// `display::look_applies`; `"None"` for none), parsed on first use. `None` when the
/// combination was not baked (an HDR display, or a view the display does not offer).
pub(crate) fn pipeline(
    display: &str,
    view: &str,
    look: &str,
) -> Option<Result<&'static Pipeline, String>> {
    let s = sections()
        .iter()
        .find(|s| s.display == display && s.view == view && s.look == look)?;
    Some(
        s.parsed
            .get_or_init(|| Pipeline::parse(s.body))
            .as_ref()
            .map_err(Clone::clone),
    )
}

/// Every baked `(display, view, look)`.
#[cfg(test)]
pub(crate) fn baked_combinations()
-> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    sections().iter().map(|s| (s.display, s.view, s.look))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_baked_pipeline_parses() {
        let mut n = 0;
        for (d, v, l) in baked_combinations() {
            let p = pipeline(d, v, l).unwrap();
            assert!(p.is_ok(), "{d}|{v}|{l}: {}", p.unwrap_err());
            n += 1;
        }
        // 4 SDR displays; every view of frap's enum they offer; every look Blender applies.
        assert!(n >= 150, "{n} pipelines");
        assert!(pipeline("sRGB", "AgX", "None").is_some());
        assert!(pipeline("sRGB", "AgX", "Medium Contrast").is_none());
        assert!(pipeline("Rec.2100-PQ", "Standard", "None").is_none());
        assert_eq!(pipeline("sRGB", "Raw", "None").unwrap().unwrap().len(), 0);
    }

    #[test]
    fn lut1d_forward_matches_ocio_indexing() {
        let t = [0.0, 0.25, 1.0];
        assert_eq!(lut1d_forward(&t, 0.0), 0.0);
        assert_eq!(lut1d_forward(&t, 0.5), 0.25);
        assert!((lut1d_forward(&t, 0.75) - 0.625).abs() < 1e-7);
        assert_eq!(lut1d_forward(&t, 2.0), 1.0);
        assert_eq!(lut1d_forward(&t, -1.0), 0.0);
        assert_eq!(lut1d_forward(&t, f32::NAN), 0.0);
    }

    #[test]
    fn lut1d_inverse_is_exact_and_handles_flat_ends_and_decreasing() {
        let t = [0.0, 0.0, 0.25, 1.0, 1.0];
        let inv = InvChannel::new(&t);
        assert_eq!((inv.start, inv.end), (1, 3));
        for x in [0.3f32, 0.5, 0.6, 0.7] {
            let y = lut1d_forward(&t, x);
            assert!((inv.apply(y) - x).abs() < 1e-6, "{x}");
        }
        // Flat ends invert to the end of the flat spot nearest the centre.
        assert_eq!(inv.apply(-1.0), 0.25);
        assert_eq!(inv.apply(2.0), 0.75);
        // Decreasing, with a reversal that gets flattened.
        let d = [1.0, 0.5, 0.6, 0.0];
        let inv = InvChannel::new(&d);
        assert_eq!(inv.flip, -1.0);
        assert!((inv.apply(0.25) - (2.0 + 0.5) / 3.0).abs() < 1e-6);
    }

    #[test]
    fn lut1d_text_parses() {
        let l = Lut1d::parse("TITLE \"x\"\n# c\nLUT_1D_SIZE 2\nCHANNELS 1\n0\n1\n").unwrap();
        assert_eq!(l.tables[2], vec![0.0, 1.0]);
        let l = Lut1d::parse("LUT_1D_SIZE 2\nCHANNELS 3\n0 0.5 1\n1 1 1\n").unwrap();
        assert_eq!(l.tables[1], vec![0.5, 1.0]);
        assert!(Lut1d::parse("LUT_1D_SIZE 3\nCHANNELS 1\n0\n1\n").is_err());
        assert!(Lut1d::parse("LUT_1D_SIZE 2\n0\n1\n").is_err());
    }

    #[test]
    fn moncurve_matches_the_srgb_transfer() {
        let enc = Moncurve::new(2.4, 0.055, false);
        let dec = Moncurve::new(2.4, 0.055, true);
        for x in [0.0f32, 0.001, 0.0031, 0.18, 0.5, 1.0, 4.0] {
            let e = enc.reverse(x);
            assert!((e - crate::spaces::linear_to_srgb(x)).abs() < 1e-4, "{x}");
            assert!((dec.forward(e) - x).abs() <= 1e-4 * x.max(1.0), "{x}");
        }
    }

    #[test]
    fn sse_fast_math_is_15_bit_accurate_and_handles_edges() {
        for i in 0..2000 {
            let x = libm::exp2f(-40.0 + i as f32 * 0.04);
            let rel = (sse_log2(x) - libm::log2f(x)).abs();
            assert!(rel < 1e-4, "log2 {x}");
            let p = sse_power(x, 0.4166667);
            assert!((p / libm::powf(x, 0.4166667) - 1.0).abs() < 1e-4, "pow {x}");
        }
        for e in -20..20 {
            let v = sse_exp2(e as f32 + 0.5);
            assert!((v / libm::exp2f(e as f32 + 0.5) - 1.0).abs() < 1e-4);
        }
        assert_eq!(sse_power(0.0, 2.0), 0.0);
        assert_eq!(sse_power(-1.0, 2.0), 0.0);
        assert_eq!(sse_power(f32::NAN, 2.0), 0.0);
        assert_eq!(sse_exp2(-200.0), 0.0);
        assert_eq!(sse_exp2(200.0), f32::INFINITY);
    }

    #[test]
    fn simple_ops_follow_ocio() {
        let range = parse_op("range 0.0 - 0.0 -").unwrap();
        assert_eq!(range.apply([-1.0, 2.0, f32::NAN]), [0.0, 2.0, 0.0]);
        let range = parse_op("range 0.0 1.0 0.0 1.0").unwrap();
        assert_eq!(range.apply([-1.0, 2.0, 0.5]), [0.0, 1.0, 0.5]);
        let range = parse_op("range 0.0 2.0 0.0 1.0").unwrap();
        assert_eq!(range.apply([1.0, 4.0, -1.0]), [0.5, 1.0, 0.0]);
        let close = |a: [f32; 3], b: [f32; 3]| {
            (0..3).all(|i| (a[i] - b[i]).abs() <= 1e-4 * b[i].abs().max(1.0))
        };
        let exp = parse_op("exponent 2.0 2.0 2.0 clamp inv").unwrap();
        assert!(close(exp.apply([4.0, -1.0, 0.0]), [2.0, 0.0, 0.0]));
        let exp = parse_op("exponent 2.0 2.0 2.0 mirror fwd").unwrap();
        assert!(close(exp.apply([2.0, -2.0, 0.0]), [4.0, -4.0, 0.0]));
        let log = parse_op("log 10.0 fwd").unwrap();
        let v = log.apply([100.0, 0.0, 1.0]);
        assert!((v[0] - 2.0).abs() < 1e-4 && v[2].abs() < 1e-4);
        assert!((v[1] - libm::log10f(f32::MIN_POSITIVE)).abs() < 1e-3);
        let alog = parse_op("log 10.0 inv").unwrap();
        assert!((alog.apply([2.0, 0.0, -1.0])[0] - 100.0).abs() < 1e-2);
        let m = parse_op("matrix 1 2 3 4 5 6 7 8 9 0.5 0 -1").unwrap();
        assert_eq!(m.apply([1.0, 0.0, 1.0]), [4.5, 10.0, 15.0]);
        assert!(parse_op("matrix 1 2").is_err());
        assert!(parse_op("lut3d nope").is_err());
        assert!(parse_op("frobnicate").is_err());
        assert!(parse_op("cdl 1 1 1 0 0 0 1 1 1 1 noclamp inv").is_err());
    }
}
