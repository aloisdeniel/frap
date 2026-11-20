// SPDX-License-Identifier: GPL-3.0-or-later
//! ACES 1.3 and ACES 2.0: the baked op pipelines (`assets/luts/aces.ops`, written by
//! `tools/blender/bake_view_luts.py` and checked on held-out colours, §19.5) and the OCIO ops
//! they are made of. Owned by WP 5.3 (M5).
//!
//! Blender's ACES views are OCIO `BuiltinTransform`s, which OCIO expands into fixed functions:
//! ACES 1.3 ("ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - SDR-VIDEO_1.0") is glow, red modifier,
//! two log-space B-spline tone curves (`GradingRGBCurve`) and the dark-to-dim surround; ACES 2.0
//! is the monolithic `ACES_OUTPUT_TRANSFORM_20` (Hellwig JMh tone scale, chroma and gamut
//! compression, [`aces2`]); the "Reference Gamut Compression" looks are `ACES_GAMUT_COMP_13`.
//! Ported from OCIO 2.5 (`ops/fixedfunction/FixedFunctionOpCPU.cpp`, `ops/fixedfunction/ACES2/`,
//! `ops/gradingrgbcurve/GradingBSplineCurve.cpp`; BSD-3-Clause, Copyright Contributors to the
//! OpenColorIO Project), forward direction.
//!
//! None of these views is a single LUT, so [`BAKED`] (the `lut::baked` list) stays empty.

pub mod aces2;

pub use aces2::OutputTransform20;

use crate::display::ocio::{DataFile, OpsFile};
use crate::lut::BakedLut;

/// ACES 1.3 and ACES 2.0 single-LUT views: none (see the module docs).
pub const BAKED: &[BakedLut] = &[];

/// The ACES op pipelines: every display × ACES view × look.
pub(crate) const PIPELINES: OpsFile = include_str!("../../../assets/luts/aces.ops");

/// LUT data of the ACES pipelines: none (they are all analytic).
pub(crate) const DATA: &[DataFile] = &[];

/// `CalcSatWeight`: a saturation measure, safe for negatives and dark noise.
#[inline]
fn sat_weight(r: f32, g: f32, b: f32) -> f32 {
    const NOISE_LIMIT: f32 = 1e-2;
    let min = r.min(g.min(b));
    let max = r.max(g.max(b));
    (max.max(1e-10) - min.max(1e-10)) / max.max(NOISE_LIMIT)
}

/// `Renderer_ACES_Glow03_Fwd` with the ACES 1.0 constants (gain 0.05, mid 0.08).
pub fn glow_10(p: [f32; 3]) -> [f32; 3] {
    const GLOW_GAIN: f32 = 0.05;
    const GLOW_MID: f32 = 0.08;
    let [r, g, b] = p;
    // rgbToYC
    let chroma = libm::sqrtf(b * (b - g) + g * (g - r) + r * (r - b));
    let yc = (b + g + r + 1.75 * chroma) / 3.0;
    let sat = sat_weight(r, g, b);
    // SigmoidShaper
    let x = (sat - 0.4) * 5.0;
    let sign = libm::copysignf(1.0, x);
    let t = (1.0 - 0.5 * sign * x).max(0.0);
    let s = (1.0 + sign * (1.0 - t * t)) * 0.5;
    let gain = GLOW_GAIN * s;
    let out_gain = if yc >= GLOW_MID * 2.0 {
        0.0
    } else if yc <= GLOW_MID * 2.0 / 3.0 {
        gain
    } else {
        gain * (GLOW_MID / yc - 0.5)
    };
    let f = 1.0 + out_gain;
    [r * f, g * f, b * f]
}

/// `CalcHueWeight`: the quadratic B-spline hue window around red.
fn hue_weight(r: f32, g: f32, b: f32, inv_width: f32) -> f32 {
    const SQRT3: f32 = 1.732_050_8;
    let a = 2.0 * r - (g + b);
    let bb = SQRT3 * (g - b);
    let hue = libm::atan2f(bb, a);
    let knot_coord = hue * inv_width + 2.0;
    let j = knot_coord as i32;
    const M: [[f32; 4]; 4] = [
        [0.25, 0.00, 0.00, 0.00],
        [-0.75, 0.75, 0.75, 0.25],
        [0.75, -1.50, 0.00, 1.00],
        [-0.25, 0.75, -0.75, 0.25],
    ];
    if (0..4).contains(&j) {
        let t = knot_coord - j as f32;
        let c = M[j as usize];
        c[3] + t * (c[2] + t * (c[1] + t * c[0]))
    } else {
        0.0
    }
}

/// `Renderer_ACES_RedMod10_Fwd` (scale 0.82, pivot 0.03, width 135°).
pub fn red_mod_10(p: [f32; 3]) -> [f32; 3] {
    const ONE_MINUS_SCALE: f32 = 1.0 - 0.82;
    const PIVOT: f32 = 0.03;
    const INV_WIDTH: f32 = 1.697_652_7;
    let [mut r, g, b] = p;
    let f_h = hue_weight(r, g, b, INV_WIDTH);
    if f_h > 0.0 {
        let f_s = sat_weight(r, g, b);
        r += f_h * f_s * (PIVOT - r) * ONE_MINUS_SCALE;
    }
    [r, g, b]
}

/// `Renderer_ACES_DarkToDim10_Fwd` (gamma 0.9811, AP1 luminance).
pub fn dark_to_dim_10(p: [f32; 3]) -> [f32; 3] {
    const GAMMA_MINUS_ONE: f32 = 0.9811 - 1.0;
    let y = (0.272_228_72 * p[0] + 0.674_081_8 * p[1] + 0.053_689_517 * p[2]).max(1e-10);
    let f = libm::powf(y, GAMMA_MINUS_ONE);
    p.map(|v| v * f)
}

/// `Renderer_ACES_GamutComp13_Fwd`: the ACES 1.3 reference gamut compression.
#[derive(Clone, Debug, PartialEq)]
pub struct GamutComp13 {
    thr: [f32; 3],
    scale: [f32; 3],
    power: f32,
}

impl GamutComp13 {
    /// From `limCyan limMagenta limYellow thrCyan thrMagenta thrYellow power`.
    pub fn new(params: &[f64]) -> Result<GamutComp13, String> {
        if params.len() != 7 {
            return Err("aces_gamut_comp_13: expected 7 parameters".into());
        }
        let p = params.iter().map(|v| *v as f32).collect::<Vec<_>>();
        let power = p[6];
        let scale = |lim: f32, thr: f32| {
            (lim - thr)
                / libm::powf(
                    libm::powf((1.0 - thr) / (lim - thr), -power) - 1.0,
                    1.0 / power,
                )
        };
        Ok(GamutComp13 {
            thr: [p[3], p[4], p[5]],
            scale: [scale(p[0], p[3]), scale(p[1], p[4]), scale(p[2], p[5])],
            power,
        })
    }

    /// Compresses each channel's distance from the achromatic axis beyond its threshold.
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let ach = p[0].max(p[1].max(p[2]));
        [0, 1, 2].map(|c| {
            if ach == 0.0 {
                return 0.0;
            }
            let dist = (ach - p[c]) / ach.abs();
            if dist < self.thr[c] {
                return p[c];
            }
            let (thr, scale) = (self.thr[c], self.scale[c]);
            let nd = (dist - thr) / scale;
            let pw = libm::powf(nd, self.power);
            let compr = thr + scale * nd / libm::powf(1.0 + pw, 1.0 / self.power);
            ach - compr * ach.abs()
        })
    }
}

/// One fitted curve of a `GradingBSplineCurve` (`KnotsCoefs`): knots and quadratic segments.
#[derive(Clone, Debug, PartialEq)]
struct Spline {
    knots: Vec<f32>,
    a: Vec<f32>,
    b: Vec<f32>,
    c: Vec<f32>,
}

/// `EstimateRGBSlopes`.
fn estimate_slopes(pts: &[(f32, f32)]) -> Vec<f32> {
    let n = pts.len();
    let mut secant_slope = Vec::with_capacity(n);
    let mut secant_len = Vec::with_capacity(n);
    for i in 0..n - 1 {
        let dx = pts[i + 1].0 - pts[i].0;
        let dy = pts[i + 1].1 - pts[i].1;
        secant_slope.push(dy / dx);
        secant_len.push(libm::sqrtf(dx * dx + dy * dy));
    }
    if n == 2 {
        return vec![secant_slope[0], secant_slope[0]];
    }
    let mut i = 0;
    loop {
        let mut j = i;
        let mut dl = secant_len[i];
        while j < n - 2 && (secant_slope[j + 1] - secant_slope[j]).abs() < 1e-6 {
            dl += secant_len[j + 1];
            j += 1;
        }
        for len in &mut secant_len[i..=j] {
            *len = dl;
        }
        if j >= n - 3 {
            break;
        }
        i = j + 1;
    }
    let mut slopes = vec![0.0];
    for k in 1..n - 1 {
        slopes.push(
            (secant_len[k] * secant_slope[k] + secant_len[k - 1] * secant_slope[k - 1])
                / (secant_len[k] + secant_len[k - 1]),
        );
    }
    slopes.push((0.5 * (3.0 * secant_slope[n - 2] - slopes[n - 2])).max(0.01));
    slopes[0] = (0.5 * (3.0 * secant_slope[0] - slopes[1])).max(0.01);
    slopes
}

/// `FitRGBSpline`.
fn fit_spline(pts: &[(f32, f32)], slopes: &[f32]) -> Spline {
    let mut s = Spline {
        knots: vec![pts[0].0],
        a: Vec::new(),
        b: Vec::new(),
        c: Vec::new(),
    };
    for i in 0..pts.len() - 1 {
        let (xi, yi) = pts[i];
        let (xi1, yi1) = pts[i + 1];
        let dx = xi1 - xi;
        let secant = (yi1 - yi) / dx;
        if ((slopes[i] + slopes[i + 1]) - 2.0 * secant).abs() < 1e-6 {
            s.c.push(yi);
            s.b.push(slopes[i]);
            s.a.push(0.5 * (slopes[i + 1] - slopes[i]) / dx);
        } else {
            let aa = slopes[i] - secant;
            let bb = slopes[i + 1] - secant;
            let ksi = if aa * bb >= 0.0 {
                (xi + xi1) * 0.5
            } else if aa.abs() > bb.abs() {
                xi1 + aa * dx / (slopes[i + 1] - slopes[i])
            } else {
                xi + bb * dx / (slopes[i + 1] - slopes[i])
            };
            let s_bar =
                (2.0 * secant - slopes[i + 1]) + (slopes[i + 1] - slopes[i]) * (ksi - xi) / dx;
            let eta = (s_bar - slopes[i]) / (ksi - xi);
            s.c.push(yi);
            s.b.push(slopes[i]);
            s.a.push(0.5 * eta);
            s.c.push(yi + slopes[i] * (ksi - xi) + 0.5 * eta * (ksi - xi) * (ksi - xi));
            s.b.push(s_bar);
            s.a.push(0.5 * (slopes[i + 1] - s_bar) / (xi1 - ksi));
            s.knots.push(ksi);
        }
        s.knots.push(xi1);
    }
    s
}

/// `AdjustRGBSlopes`: rescales slopes whose fit would give a negative mid slope.
fn adjust_slopes(pts: &[(f32, f32)], slopes: &mut [f32], knots: &[f32]) -> bool {
    let mut done = false;
    let mut i = 0;
    for &ksi in knots {
        if i + 1 >= pts.len() {
            break;
        }
        if pts[i].0 != ksi {
            let (xi, yi) = pts[i];
            let (xi1, yi1) = pts[i + 1];
            let s_bar = (2.0 * (yi1 - yi) - (ksi - xi) * slopes[i] - (xi1 - ksi) * slopes[i + 1])
                / (xi1 - xi);
            if s_bar < 0.0 {
                done = true;
                let secant = (yi1 - yi) / (xi1 - xi);
                let blend = ((ksi - xi) * slopes[i] + (xi1 - ksi) * slopes[i + 1]) / (xi1 - xi);
                let mut aim = 0.01 * 0.5 * (slopes[i] + slopes[i + 1]);
                if aim > secant {
                    aim = secant;
                }
                let adjust = (2.0 * secant - aim) / blend;
                slopes[i] *= adjust;
                slopes[i + 1] *= adjust;
            }
            i += 1;
        }
    }
    done
}

impl Spline {
    /// `computeKnotsAndCoefsForRGBCurve`: `None` for an identity curve.
    fn new(pts: &[(f32, f32)], given_slopes: &[f32]) -> Option<Spline> {
        let default_slopes = given_slopes.iter().all(|s| *s == 0.0);
        if pts.len() < 2 || (pts.iter().all(|(x, y)| x == y) && default_slopes) {
            return None;
        }
        let mut slopes = if !default_slopes && given_slopes.len() == pts.len() {
            given_slopes.to_vec()
        } else {
            estimate_slopes(pts)
        };
        let mut s = fit_spline(pts, &slopes);
        if adjust_slopes(pts, &mut slopes, &s.knots) {
            s = fit_spline(pts, &slopes);
        }
        Some(s)
    }

    /// `KnotsCoefs::evalCurve`: linear extrapolation outside the knots.
    fn eval(&self, x: f32) -> f32 {
        let n = self.knots.len();
        let (kn_start, kn_end) = (self.knots[0], self.knots[n - 1]);
        if x <= kn_start {
            return (x - kn_start) * self.b[0] + self.c[0];
        }
        let sets = self.a.len();
        if x >= kn_end {
            let (a, b, c) = (self.a[sets - 1], self.b[sets - 1], self.c[sets - 1]);
            let t = kn_end - self.knots[n - 2];
            let slope = 2.0 * a * t + b;
            let offs = (a * t + b) * t + c;
            return (x - kn_end) * slope + offs;
        }
        let mut i = 0;
        while i < n - 2 {
            if x < self.knots[i + 1] {
                break;
            }
            i += 1;
        }
        let t = x - self.knots[i];
        (self.a[i] * t + self.b[i]) * t + self.c[i]
    }
}

/// `GradingRGBCurveTransform`, log style, forward: per-channel curves then the master curve.
#[derive(Clone, Debug, PartialEq)]
pub struct RgbCurveLog {
    curves: [Option<Spline>; 4],
}

impl RgbCurveLog {
    /// From four `n (x y slope)×n` groups (red, green, blue, master), as the bake writes them.
    pub fn new(v: &[f64]) -> Result<RgbCurveLog, String> {
        let mut at = 0usize;
        let mut curves: [Option<Spline>; 4] = [None, None, None, None];
        for curve in &mut curves {
            let n = *v.get(at).ok_or("curve_log: truncated")? as usize;
            at += 1;
            let body = v.get(at..at + 3 * n).ok_or("curve_log: truncated")?;
            at += 3 * n;
            let pts: Vec<(f32, f32)> = body.chunks(3).map(|c| (c[0] as f32, c[1] as f32)).collect();
            let slopes: Vec<f32> = body.chunks(3).map(|c| c[2] as f32).collect();
            if pts.windows(2).any(|w| w[1].0 <= w[0].0) {
                return Err("curve_log: control points must increase in x".into());
            }
            *curve = Spline::new(&pts, &slopes);
        }
        if at != v.len() {
            return Err("curve_log: trailing values".into());
        }
        Ok(RgbCurveLog { curves })
    }

    /// Evaluates the curves (identity curves pass values through).
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let eval = |c: &Option<Spline>, x: f32| c.as_ref().map_or(x, |s| s.eval(x));
        let o = [0, 1, 2].map(|i| eval(&self.curves[i], p[i]));
        o.map(|x| eval(&self.curves[3], x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glow_and_red_mod_leave_greys_alone_and_touch_saturated_reds() {
        // A grey above twice the glow midpoint is untouched, and greys never get the red mod.
        assert_eq!(glow_10([0.5, 0.5, 0.5]), [0.5, 0.5, 0.5]);
        assert_eq!(red_mod_10([0.5, 0.5, 0.5]), [0.5, 0.5, 0.5]);
        // Dark saturated colours glow (gain up to 5 %).
        let g = glow_10([0.02, 0.0, 0.0]);
        assert!(g[0] > 0.02 && g[0] <= 0.02 * 1.05 + 1e-7, "{g:?}");
        // Saturated red is pulled towards the pivot.
        let r = red_mod_10([1.0, 0.1, 0.1]);
        assert!(r[0] < 1.0 && r[1] == 0.1 && r[2] == 0.1, "{r:?}");
        // Blue is outside the hue window.
        assert_eq!(red_mod_10([0.1, 0.1, 1.0]), [0.1, 0.1, 1.0]);
    }

    #[test]
    fn dark_to_dim_scales_by_luminance_power() {
        let o = dark_to_dim_10([0.18, 0.18, 0.18]);
        let y: f32 = 0.18 * (0.272_228_72 + 0.674_081_8 + 0.053_689_517);
        let want = 0.18 * libm::powf(y, 0.9811 - 1.0);
        assert!((o[0] - want).abs() < 1e-6);
    }

    #[test]
    fn gamut_comp_moves_only_out_of_gamut_values() {
        let gc = GamutComp13::new(&[1.147, 1.264, 1.312, 0.815, 0.803, 0.88, 1.2]).unwrap();
        // Inside the thresholds nothing moves.
        assert_eq!(gc.apply([0.5, 0.4, 0.3]), [0.5, 0.4, 0.3]);
        assert_eq!(gc.apply([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
        // A negative channel is pulled back towards the achromatic axis.
        let o = gc.apply([1.0, -0.2, 0.5]);
        assert!(o[1] > -0.2 && o[1] < 0.2, "{o:?}");
        assert_eq!(o[0], 1.0);
    }

    #[test]
    fn spline_interpolates_its_control_points_with_given_slopes() {
        // ACES 1.3's first curve (RRT tone scale in log10 space).
        let pts = [
            (-5.260_177_6, -4.0),
            (-3.755_027_5, -3.578_688_4),
            (-2.249_877_5, -1.821_313_3),
            (-0.744_727_5, 0.681_241_2),
            (1.061_452_5, 2.874_577_5),
            (2.867_632_4, 3.834_062),
            (4.673_812_4, 4.0),
        ];
        let slopes = [
            0.0,
            0.559_826_85,
            1.775_322_4,
            1.55,
            0.878_701_7,
            0.183_744_62,
            0.0,
        ];
        let s = Spline::new(&pts, &slopes).unwrap();
        for (x, y) in pts {
            assert!((s.eval(x) - y).abs() < 1e-4, "{x}: {} vs {y}", s.eval(x));
        }
        // Flat (slope 0) extrapolation at both ends.
        assert!((s.eval(-8.0) + 4.0).abs() < 1e-6);
        assert!((s.eval(8.0) - 4.0).abs() < 1e-5);
        // Monotonic in between.
        let mut last = -5.0;
        for i in 0..=200 {
            let y = s.eval(-5.26 + i as f32 * 0.05);
            assert!(y >= last - 1e-6);
            last = y;
        }
        // Identity curves are skipped; estimated slopes still interpolate the points.
        assert!(Spline::new(&[(0.0, 0.0), (1.0, 1.0)], &[0.0, 0.0]).is_none());
        let e = Spline::new(&[(0.0, 0.0), (0.5, 0.7), (1.0, 1.0)], &[0.0; 3]).unwrap();
        assert!((e.eval(0.5) - 0.7).abs() < 1e-6);
    }
}
