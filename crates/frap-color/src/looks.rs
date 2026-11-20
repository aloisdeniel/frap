// SPDX-License-Identifier: GPL-3.0-or-later
//! Looks (`view_settings.look`): which looks each view transform offers in Blender 5.2.4's OCIO
//! config, and the colour-grading ops the looks are made of. Owned by WP 5.3 (M5).
//!
//! A look is applied by Blender as an OCIO `LookTransform` from scene linear into the look's
//! process space ("Filmic Log" for the generic contrast looks, "AgX Log" for the AgX and
//! False Color looks, "ACES2065-1" for the ACES gamut compression), followed by the view's
//! `DisplayViewTransform` from that space (`create_ocio_display_transform`). The baked op
//! pipelines (`assets/luts/*.ops`) hold that whole chain per view × look; the grading ops it
//! contains are ported here from OCIO 2.5 (`src/OpenColorIO/ops/{cdl,gradingprimary,
//! gradingtone}`, BSD-3-Clause, Copyright Contributors to the OpenColorIO Project): CDL,
//! GradingPrimary (log style) and GradingTone (log style), forward direction.

use crate::display::ocio::sse_power;
use crate::display::{ColorError, ViewTransform};

/// The looks of Blender 5.2.4's `config.ocio` (besides "None"). A look named `"<View> - <Name>"`
/// applies only to the view `<View>`; the others apply to views that have no view-specific look.
pub const LOOKS: &[&str] = &[
    "Very High Contrast",
    "High Contrast",
    "Medium High Contrast",
    "Medium Contrast",
    "Medium Low Contrast",
    "Low Contrast",
    "Very Low Contrast",
    "AgX - Punchy",
    "AgX - Greyscale",
    "AgX - Very High Contrast",
    "AgX - High Contrast",
    "AgX - Medium High Contrast",
    "AgX - Base Contrast",
    "AgX - Medium Low Contrast",
    "AgX - Low Contrast",
    "AgX - Very Low Contrast",
    "False Color - Punchy",
    "False Color - Greyscale",
    "False Color - Very High Contrast",
    "False Color - High Contrast",
    "False Color - Medium High Contrast",
    "False Color - Base Contrast",
    "False Color - Medium Low Contrast",
    "False Color - Low Contrast",
    "False Color - Very Low Contrast",
    "ACES 1.3 - Reference Gamut Compression",
    "ACES 2.0 - Reference Gamut Compression",
];

fn look_view(look: &str) -> &str {
    look.split_once(" - ").map(|(v, _)| v).unwrap_or("")
}

/// Whether Blender applies `look` with `view` (port of `colormanage_use_look` and
/// `view_filter_for_look`, `imbuf/intern/colormanagement.cc`). `"None"` never applies.
pub fn look_applies(view: ViewTransform, look: &str) -> Result<bool, ColorError> {
    if look == "None" {
        return Ok(false);
    }
    if !LOOKS.contains(&look) {
        return Err(ColorError::UnknownLook(look.to_owned()));
    }
    let name = view.name();
    let filter = if LOOKS.iter().any(|l| look_view(l) == name) {
        name
    } else {
        let short = name.split_once(" - ").map(|(s, _)| s).unwrap_or("");
        if !short.is_empty() && LOOKS.iter().any(|l| look_view(l) == short) {
            short
        } else {
            ""
        }
    };
    Ok(look_view(look) == filter)
}

/// The looks Blender offers for `view` (`view_settings.look` enum items, "None" first).
pub fn looks_for(view: ViewTransform) -> Vec<&'static str> {
    std::iter::once("None")
        .chain(
            LOOKS
                .iter()
                .copied()
                .filter(|l| look_applies(view, l).unwrap_or(false)),
        )
        .collect()
}

/// The UI name of a look (`ocio::Look::ui_name`): the part after `"<View> - "` for a
/// view-specific look, the whole name otherwise (`"AgX - Punchy"` → `"Punchy"`).
pub fn ui_name(look: &str) -> &str {
    look.split_once(" - ").map(|(_, n)| n).unwrap_or(look)
}

/// The look `view_settings.look` holds after `view_transform` becomes `view` (port of
/// `IMB_colormanagement_look_validate_for_view`, blender/source/blender/imbuf/intern/
/// colormanagement.cc, called by `rna_ColorManagedViewSettings_view_transform_set`): an unknown
/// name is kept; a look `view` offers is kept; otherwise the first look of the config with the
/// same UI name that `view` offers (`AgX - Low Contrast` → `Low Contrast` on Standard,
/// `High Contrast` → `False Color - High Contrast` on False Color); otherwise `"None"`.
pub fn validate_for_view(view: ViewTransform, look: &str) -> &str {
    if look == "None" {
        return "None";
    }
    let Some(&known) = LOOKS.iter().find(|l| **l == look) else {
        return look;
    };
    if look_applies(view, known).unwrap_or(false) {
        return known;
    }
    let ui = ui_name(known);
    LOOKS
        .iter()
        .copied()
        .find(|l| ui_name(l) == ui && look_applies(view, l).unwrap_or(false))
        .unwrap_or("None")
}

/// The look Blender actually renders `look` with on `view`: the look itself, or `"None"` when
/// the view ignores it. ("Medium Contrast" is an empty group, but Blender still converts into
/// its process space, Filmic Log, and back, which clips and desaturates highlights; it is a
/// real look.)
pub fn effective_look(view: ViewTransform, look: &str) -> Result<&str, ColorError> {
    if look_applies(view, look)? {
        Ok(look)
    } else {
        Ok("None")
    }
}

const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

#[inline]
fn apply_saturation(p: [f32; 3], sat: f32) -> [f32; 3] {
    let luma = p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2];
    p.map(|v| luma + sat * (v - luma))
}

/// OCIO `Clamp(v, 0, 1)` (NaN → 0).
#[inline]
fn clamp01(v: f32) -> f32 {
    if v > 1.0 {
        1.0
    } else if v >= 0.0 {
        v
    } else {
        0.0
    }
}

/// `CDLTransform` (`CDLRendererFwd`), forward.
#[derive(Clone, Debug, PartialEq)]
pub struct Cdl {
    slope: [f32; 3],
    offset: [f32; 3],
    power: [f32; 3],
    saturation: f32,
    clamp: bool,
}

impl Cdl {
    /// From `slope[3] offset[3] power[3] sat` and the clamp style (`CDL_V1_2`/ASC vs no-clamp).
    pub fn new(v: &[f64], clamp: bool) -> Result<Cdl, String> {
        if v.len() != 10 {
            return Err("cdl: expected 10 values".into());
        }
        let f = |i: usize| v[i] as f32;
        Ok(Cdl {
            slope: [f(0), f(1), f(2)],
            offset: [f(3), f(4), f(5)],
            power: [f(6), f(7), f(8)],
            saturation: f(9),
            clamp,
        })
    }

    /// Slope, offset, power, saturation (and the clamps of the ASC style).
    #[inline]
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let mut o = [0, 1, 2].map(|c| p[c] * self.slope[c] + self.offset[c]);
        if self.clamp {
            o = [0, 1, 2].map(|c| sse_power(clamp01(o[c]), self.power[c]));
        } else {
            // Negative values pass through the power unchanged; NaNs become 0.
            o = [0, 1, 2].map(|c| {
                let v = o[c];
                if v.is_nan() {
                    0.0
                } else if v < 0.0 {
                    v
                } else {
                    sse_power(v, self.power[c])
                }
            });
        }
        o = apply_saturation(o, self.saturation);
        if self.clamp { o.map(clamp01) } else { o }
    }
}

/// `GradingPrimaryTransform`, log style, forward (`GradingPrimaryLogFwdOpCPU` with the
/// `GradingPrimaryPreRender` values).
#[derive(Clone, Debug, PartialEq)]
pub struct GradingPrimaryLog {
    brightness: [f32; 3],
    contrast: [f32; 3],
    gamma: [f32; 3],
    pivot: f32,
    saturation: f32,
    pivot_black: f32,
    pivot_white: f32,
    clamp_black: f32,
    clamp_white: f32,
    gamma_identity: bool,
    bypass: bool,
}

impl GradingPrimaryLog {
    /// From `brightness[rgbm] contrast[rgbm] gamma[rgbm] saturation pivot pivotBlack
    /// pivotWhite clampBlack clampWhite` (18 values, as the bake writes them).
    pub fn new(v: &[f64]) -> GradingPrimaryLog {
        let (b, c, g) = (&v[0..4], &v[4..8], &v[8..12]);
        let brightness = [0, 1, 2].map(|i| ((b[3] + b[i]) * 6.25 / 1023.0) as f32);
        let contrast = [0, 1, 2].map(|i| (c[3] * c[i]) as f32);
        let gamma = [0, 1, 2].map(|i| (1.0 / (g[3] * g[i])) as f32);
        let (saturation, pivot) = (v[12], v[13]);
        let (clamp_black, clamp_white) = (v[16], v[17]);
        let gamma_identity = gamma == [1.0; 3];
        // `GradingPrimary::NoClampBlack()` / `NoClampWhite()`.
        let bypass = saturation == 1.0
            && clamp_black == -f64::MAX
            && clamp_white == f64::MAX
            && gamma_identity
            && brightness == [0.0; 3]
            && contrast == [1.0; 3];
        GradingPrimaryLog {
            brightness,
            contrast,
            gamma,
            pivot: (0.5 + pivot * 0.5) as f32,
            saturation: saturation as f32,
            pivot_black: v[14] as f32,
            pivot_white: v[15] as f32,
            clamp_black: clamp_black as f32,
            clamp_white: clamp_white as f32,
            gamma_identity,
            bypass,
        }
    }

    /// Brightness, contrast around the pivot, gamma between the pivots, saturation, clamp.
    #[inline]
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        if self.bypass {
            return p;
        }
        let mut o = [0, 1, 2].map(|c| {
            let v = p[c] + self.brightness[c];
            (v - self.pivot) * self.contrast[c] + self.pivot
        });
        if !self.gamma_identity {
            let range = self.pivot_white - self.pivot_black;
            o = [0, 1, 2].map(|c| {
                let d = o[c] - self.pivot_black;
                sse_power(d.abs() / range, self.gamma[c]) * libm::copysignf(1.0, d) * range
                    + self.pivot_black
            });
        }
        if self.saturation != 1.0 {
            o = apply_saturation(o, self.saturation);
        }
        o.map(|v| v.max(self.clamp_black).min(self.clamp_white))
    }
}

/// One zone of a `GradingTone` (`GradingRGBMSW`): red, green, blue, master, start, width.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgbmsw {
    rgbm: [f64; 4],
    start: f64,
    width: f64,
}

impl Rgbmsw {
    fn from(v: &[f64]) -> Rgbmsw {
        Rgbmsw {
            rgbm: [v[0], v[1], v[2], v[3]],
            start: v[4],
            width: v[5],
        }
    }

    fn is_identity(&self) -> bool {
        self.rgbm == [1.0; 4]
    }

    /// `GetChannelValue` (0..3 = R, G, B, M).
    fn channel(&self, c: usize) -> f32 {
        self.rgbm[c] as f32
    }
}

const M: usize = 3;

/// `setOnLimit`: `val < limit ? below : above`.
#[inline]
fn on_limit(val: f32, limit: f32, below: f32, above: f32) -> f32 {
    if val < limit { below } else { above }
}

/// `GradingToneTransform`, log style, forward (`GradingToneFwdOpCPU` with the
/// `GradingTonePreRender` values, `GradingTone.cpp`).
#[derive(Clone, Debug, PartialEq)]
pub struct GradingToneLog {
    blacks: Rgbmsw,
    shadows: Rgbmsw,
    midtones: Rgbmsw,
    highlights: Rgbmsw,
    whites: Rgbmsw,
    scontrast: f64,
    bypass: bool,
    mid_x: [[f32; 6]; 4],
    mid_y: [[f32; 6]; 4],
    mid_m: [[f32; 6]; 4],
    hs_x: [[[f32; 3]; 4]; 2],
    hs_y: [[[f32; 3]; 4]; 2],
    hs_m: [[[f32; 2]; 4]; 2],
    wb_x: [[[f32; 2]; 4]; 2],
    wb_y: [[[f32; 2]; 4]; 2],
    wb_m: [[[f32; 2]; 4]; 2],
    wb_gain: [[f32; 4]; 2],
    sc_x: [[f32; 4]; 2],
    sc_y: [[f32; 4]; 2],
    sc_m: [[f32; 2]; 2],
    pivot: f32,
    highlights_start: f64,
    highlights_width: f64,
    shadows_start: f64,
    shadows_width: f64,
    whites_start: f64,
    whites_width: f64,
    blacks_start: f64,
    blacks_width: f64,
}

#[allow(clippy::too_many_arguments)]
fn faux_cubic_fwd(t: f64, x0: f64, x2: f64, y0: f64, y2: f64, m0: f64, m2: f64, x1: f64) -> f64 {
    let y1 = (0.5 / ((x2 - x1) + (x1 - x0)))
        * ((2. * y0 + m0 * (x1 - x0)) * (x2 - x1) + (2. * y2 - m2 * (x2 - x1)) * (x1 - x0));
    let tl = (t - x0) / (x1 - x0);
    let tr = (t - x1) / (x2 - x1);
    let fl = y0 * (1. - tl * tl) + y1 * tl * tl + m0 * (1. - tl) * tl * (x1 - x0);
    let fr = y1 * (1. - tr) * (1. - tr) + y2 * (2. - tr) * tr + m2 * (tr - 1.) * tr * (x2 - x1);
    let mut res = if t < x1 { fl } else { fr };
    if t < x0 {
        res = y0 + (t - x0) * m0;
    }
    if t > x2 {
        res = y2 + (t - x2) * m2;
    }
    res
}

#[allow(clippy::too_many_arguments)]
fn faux_cubic_rev(t: f64, x0: f64, x2: f64, y0: f64, y2: f64, m0: f64, m2: f64, x1: f64) -> f64 {
    let y1 = (0.5 / ((x2 - x1) + (x1 - x0)))
        * ((2. * y0 + m0 * (x1 - x0)) * (x2 - x1) + (2. * y2 - m2 * (x2 - x1)) * (x1 - x0));
    let cl = y0 - t;
    let bl = m0 * (x1 - x0);
    let al = y1 - y0 - m0 * (x1 - x0);
    let discrim_l = libm::sqrt(bl * bl - 4. * al * cl);
    let out_l = (2. * cl) / (-discrim_l - bl) * (x1 - x0) + x0;
    let cr = y1 - t;
    let br = 2. * y2 - 2. * y1 - m2 * (x2 - x1);
    let ar = y1 - y2 + m2 * (x2 - x1);
    let discrim_r = libm::sqrt(br * br - 4. * ar * cr);
    let out_r = (2. * cr) / (-discrim_r - br) * (x2 - x1) + x1;
    let mut res = if t < y1 { out_l } else { out_r };
    if t < y0 {
        res = x0 + (t - y0) / m0;
    }
    if t > y2 {
        res = x2 + (t - y2) / m2;
    }
    res
}

fn highlight_fwd_eval(t: f64, start: f64, pivot: f64, val: f64) -> f64 {
    let (x0, x2) = (start, pivot);
    let x1 = x0 + (x2 - x0) * 0.5;
    let val = 2. - val;
    if val <= 1. {
        let m2 = if val < 0.01 { 0.01 } else { val };
        faux_cubic_fwd(t, x0, x2, x0, x2, 1., m2, x1)
    } else {
        let m2 = if 2. - val < 0.01 { 0.01 } else { 2. - val };
        faux_cubic_rev(t, x0, x2, x0, x2, 1., m2, x1)
    }
}

fn shadow_fwd_eval(t: f64, start: f64, pivot: f64, val: f64) -> f64 {
    let (x0, x2) = (start, pivot);
    let x1 = x0 + (x2 - x0) * 0.5;
    if val <= 1. {
        let m0 = if val < 0.01 { 0.01 } else { val };
        faux_cubic_fwd(t, x0, x2, x0, x2, m0, 1., x1)
    } else {
        let m0 = if 2. - val < 0.01 { 0.01 } else { 2. - val };
        faux_cubic_rev(t, x0, x2, x0, x2, m0, 1., x1)
    }
}

impl GradingToneLog {
    /// From `blacks shadows midtones highlights whites` (each `r g b m start width`) and
    /// `scontrast` (31 values, as the bake writes them).
    pub fn new(v: &[f64]) -> GradingToneLog {
        let mut t = GradingToneLog {
            blacks: Rgbmsw::from(&v[0..6]),
            shadows: Rgbmsw::from(&v[6..12]),
            midtones: Rgbmsw::from(&v[12..18]),
            highlights: Rgbmsw::from(&v[18..24]),
            whites: Rgbmsw::from(&v[24..30]),
            scontrast: v[30],
            bypass: false,
            mid_x: [[0.0; 6]; 4],
            mid_y: [[0.0; 6]; 4],
            mid_m: [[0.0; 6]; 4],
            hs_x: [[[0.0; 3]; 4]; 2],
            hs_y: [[[0.0; 3]; 4]; 2],
            hs_m: [[[0.0; 2]; 4]; 2],
            wb_x: [[[0.0; 2]; 4]; 2],
            wb_y: [[[0.0; 2]; 4]; 2],
            wb_m: [[[0.0; 2]; 4]; 2],
            wb_gain: [[0.0; 4]; 2],
            sc_x: [[0.0; 4]; 2],
            sc_y: [[0.0; 4]; 2],
            sc_m: [[0.0; 2]; 2],
            // `FromStyle(GRADING_LOG)`: top 1, topSC 1, bottom 0, pivot 0.4.
            pivot: 0.4,
            highlights_start: 0.0,
            highlights_width: 0.0,
            shadows_start: 0.0,
            shadows_width: 0.0,
            whites_start: 0.0,
            whites_width: 0.0,
            blacks_start: 0.0,
            blacks_width: 0.0,
        };
        t.update();
        t
    }

    /// `GradingTonePreRender::update`.
    fn update(&mut self) {
        self.bypass = self.blacks.is_identity()
            && self.shadows.is_identity()
            && self.midtones.is_identity()
            && self.highlights.is_identity()
            && self.whites.is_identity()
            && self.scontrast == 1.0;
        if self.bypass {
            return;
        }
        {
            let master = self.highlights.rgbm[M];
            let (start, pivot) = (self.highlights.start, self.highlights.width);
            let (startw, widthw) = (self.whites.start, self.whites.width);
            self.highlights_start = if start > pivot - 0.01 {
                pivot - 0.01
            } else {
                start
            };
            self.highlights_width = pivot;
            let new_start =
                highlight_fwd_eval(startw, self.highlights_start, self.highlights_width, master);
            let new_end = highlight_fwd_eval(
                startw + widthw,
                self.highlights_start,
                self.highlights_width,
                master,
            );
            self.whites_start = new_start;
            self.whites_width = new_end - new_start;
        }
        {
            let master = self.shadows.rgbm[M];
            let (start, pivot) = (self.shadows.start, self.shadows.width);
            let (startb, widthb) = (self.blacks.start, self.blacks.width);
            self.shadows_start = if start < pivot + 0.01 {
                pivot + 0.01
            } else {
                start
            };
            self.shadows_width = pivot;
            let new_start = shadow_fwd_eval(startb, self.shadows_width, self.shadows_start, master);
            let new_end = shadow_fwd_eval(
                startb - widthb,
                self.shadows_width,
                self.shadows_start,
                master,
            );
            self.blacks_start = new_start;
            self.blacks_width = new_start - new_end;
        }
        self.mids_precompute(1.0, 0.0);
        self.highlight_shadow_precompute();
        self.white_black_precompute();
        self.scontrast_precompute(1.0, 0.0, 0.4);
    }

    fn mids_precompute(&mut self, top: f32, bottom: f32) {
        const HALO: f32 = 0.4;
        for ch in 0..4 {
            let mut mid_adj = self.midtones.channel(ch).clamp(0.01, 1.99);
            if mid_adj == 1.0 {
                continue;
            }
            let x0 = bottom;
            let x5 = top;
            let max_width = (x5 - x0) * 0.95;
            let width = (self.midtones.width as f32).clamp(0.01, max_width);
            let min_cent = x0 + width * 0.51;
            let max_cent = x5 - width * 0.51;
            let center = (self.midtones.start as f32).clamp(min_cent, max_cent);
            let x1 = center - width * 0.5;
            let x4 = x1 + width;
            let x2 = x1 + (x4 - x1) * 0.25;
            let x3 = x1 + (x4 - x1) * 0.75;
            let y0 = x0;
            let m0 = 1.0f32;
            let m5 = 1.0f32;
            let min_slope = 0.1f32;
            mid_adj -= 1.0;
            mid_adj *= 1.0 - min_slope;
            let m2 = 1.0 + mid_adj;
            let m3 = 1.0 - mid_adj;
            let mut m1 = 1.0 + mid_adj * HALO;
            let mut m4 = 1.0 - mid_adj * HALO;
            if center <= (x5 + x0) * 0.5 {
                let area = (x1 - x0) * (m1 - m0) * 0.5
                    + (x2 - x1) * ((m1 - m0) + (m2 - m1) * 0.5)
                    + (center - x2) * (m2 - m0) * 0.5;
                m4 = (-0.5 * (x5 - x4) * m5
                    + (x4 - x3) * (0.5 * m3 - m5)
                    + (x3 - center) * (m3 - m5) * 0.5
                    + area)
                    / (-0.5 * (x5 - x3));
            } else {
                let area = (x5 - x4) * (m4 - m5) * 0.5
                    + (x4 - x3) * ((m4 - m5) + (m3 - m4) * 0.5)
                    + (x3 - center) * (m3 - m5) * 0.5;
                m1 = (-0.5 * (x1 - x0) * m0
                    + (x2 - x1) * (0.5 * m2 - m0)
                    + (center - x2) * (m2 - m0) * 0.5
                    + area)
                    / (-0.5 * (x2 - x0));
            }
            let y1 = y0 + (m0 + m1) * (x1 - x0) * 0.5;
            let y2 = y1 + (m1 + m2) * (x2 - x1) * 0.5;
            let y3 = y2 + (m2 + m3) * (x3 - x2) * 0.5;
            let y4 = y3 + (m3 + m4) * (x4 - x3) * 0.5;
            let y5 = y4 + (m4 + m5) * (x5 - x4) * 0.5;
            self.mid_x[ch] = [x0, x1, x2, x3, x4, x5];
            self.mid_y[ch] = [y0, y1, y2, y3, y4, y5];
            self.mid_m[ch] = [m0, m1, m2, m3, m4, m5];
        }
    }

    fn highlight_shadow_precompute(&mut self) {
        for (z, is_shadow) in [(0usize, false), (1, true)] {
            for ch in 0..4 {
                let mut val = if is_shadow {
                    self.shadows.channel(ch)
                } else {
                    self.highlights.channel(ch)
                };
                if !is_shadow {
                    val = 2.0 - val;
                }
                if val == 1.0 {
                    continue;
                }
                let start = if is_shadow {
                    self.shadows_start
                } else {
                    self.highlights_start
                } as f32;
                let pivot = if is_shadow {
                    self.shadows_width
                } else {
                    self.highlights_width
                } as f32;
                let x0 = if is_shadow { pivot } else { start };
                let x2 = if is_shadow { start } else { pivot };
                let (y0, y2) = (x0, x2);
                let x1 = x0 + (x2 - x0) * 0.5;
                let (m0, m2, y1);
                if val < 1.0 {
                    m0 = if is_shadow { val.max(0.01) } else { 1.0 };
                    m2 = if is_shadow { 1.0 } else { val.max(0.01) };
                    y1 = (0.5 / (x2 - x0))
                        * ((2.0 * y0 + m0 * (x1 - x0)) * (x2 - x1)
                            + (2.0 * y2 - m2 * (x2 - x1)) * (x1 - x0));
                } else {
                    m0 = if is_shadow {
                        (2.0 - val).max(0.01)
                    } else {
                        1.0
                    };
                    m2 = if is_shadow {
                        1.0
                    } else {
                        (2.0 - val).max(0.01)
                    };
                    y1 = (0.5 / ((x2 - x1) + (x1 - x0)))
                        * ((2.0 * y0 + m0 * (x1 - x0)) * (x2 - x1)
                            + (2.0 * y2 - m2 * (x2 - x1)) * (x1 - x0));
                }
                self.hs_x[z][ch] = [x0, x1, x2];
                self.hs_y[z][ch] = [y0, y1, y2];
                self.hs_m[z][ch] = [m0, m2];
            }
        }
    }

    fn white_black_precompute(&mut self) {
        for (z, is_black) in [(0usize, false), (1, true)] {
            for ch in 0..4 {
                let start = if is_black {
                    self.blacks_start
                } else {
                    self.whites_start
                } as f32;
                let width = if is_black {
                    self.blacks_width
                } else {
                    self.whites_width
                } as f32;
                let val = if is_black {
                    self.blacks.channel(ch)
                } else {
                    self.whites.channel(ch)
                };
                let x0 = if !is_black { start } else { start - width };
                let x1 = if !is_black { x0 + width } else { start };
                let mtest = if !is_black { val } else { 2.0 - val };
                let [mut y0, mut y1] = self.wb_y[z][ch];
                let [mut m0, mut m1] = self.wb_m[z][ch];
                if mtest < 1.0 {
                    if !is_black {
                        m0 = 1.0;
                        m1 = val.max(0.01);
                        y0 = x0;
                        y1 = y0 + (m0 + m1) * (x1 - x0) * 0.5;
                    } else {
                        m0 = (2.0 - val).max(0.01);
                        m1 = 1.0;
                        y1 = x1;
                        y0 = y1 - (m0 + m1) * (x1 - x0) * 0.5;
                    }
                } else if mtest > 1.0 {
                    if !is_black {
                        m0 = 1.0;
                        m1 = (2.0 - val).max(0.01);
                        y0 = x0;
                    } else {
                        m0 = val.max(0.01);
                        m1 = 1.0;
                        y1 = x1;
                        y0 = y1 - (m0 + m1) * (x1 - x0) * 0.5;
                    }
                    self.wb_gain[z][ch] = (m0 + m1) * 0.5;
                }
                self.wb_x[z][ch] = [x0, x1];
                self.wb_y[z][ch] = [y0, y1];
                self.wb_m[z][ch] = [m0, m1];
            }
        }
    }

    fn scontrast_precompute(&mut self, top_sc: f32, bottom: f32, pivot: f32) {
        let contrast = self.scontrast as f32;
        if contrast == 1.0 {
            return;
        }
        let contrast = if contrast > 1.0 {
            1.0 / (1.8125 - 0.8125 * contrast.min(1.99))
        } else {
            0.28125 + 0.71875 * contrast.max(0.01)
        };
        {
            let x3 = top_sc;
            let y3 = top_sc;
            let y0 = pivot + (y3 - pivot) * 0.25;
            let m0 = contrast;
            let x0 = pivot + (y0 - pivot) / m0;
            let min_width = (x3 - x0) * 0.3;
            let mut m3 = 1.0 / m0;
            let center = (y3 - y0 - m3 * x3 + m0 * x0) / (m0 - m3);
            let mut x1 = x0;
            let mut x2 = 2.0 * center - x1;
            if x2 > x3 {
                x2 = x3;
                x1 = 2.0 * center - x2;
            } else if (x2 - x1) < min_width {
                x2 = x1 + min_width;
                let new_center = (x2 + x1) * 0.5;
                m3 = (y3 - y0 + m0 * x0 - new_center * m0) / (x3 - new_center);
            }
            let y1 = y0;
            let y2 = y1 + (m0 + m3) * (x2 - x1) * 0.5;
            self.sc_x[0] = [x0, x1, x2, x3];
            self.sc_y[0] = [y0, y1, y2, y3];
            self.sc_m[0] = [m0, m3];
        }
        {
            let x0 = bottom;
            let y0 = bottom;
            let y3 = pivot - (pivot - y0) * 0.25;
            let m3 = contrast;
            let x3 = pivot - (pivot - y3) / m3;
            let min_width = (x3 - x0) * 0.3;
            let mut m0 = 1.0 / m3;
            let center = (y3 - y0 - m3 * x3 + m0 * x0) / (m0 - m3);
            let mut x2 = x3;
            let mut x1 = 2.0 * center - x2;
            if x1 < x0 {
                x1 = x0;
                x2 = 2.0 * center - x1;
            } else if (x2 - x1) < min_width {
                x1 = x2 - min_width;
                let new_center = (x2 + x1) * 0.5;
                m0 = (y3 - y0 - m3 * x3 + new_center * m3) / (new_center - x0);
            }
            let y2 = y3;
            let y1 = y2 - (m0 + m3) * (x2 - x1) * 0.5;
            self.sc_x[1] = [x0, x1, x2, x3];
            self.sc_y[1] = [y0, y1, y2, y3];
            self.sc_m[1] = [m0, m3];
        }
    }

    /// Runs `f` on the channel `ch` (R, G, B) or, for the master channel, on all three.
    #[inline]
    fn for_channel(out: &mut [f32; 3], ch: usize, f: impl Fn(f32) -> f32) {
        if ch == M {
            *out = out.map(&f);
        } else {
            out[ch] = f(out[ch]);
        }
    }

    fn mids(&self, ch: usize, out: &mut [f32; 3]) {
        let mid_adj = self.midtones.channel(ch).clamp(0.01, 1.99);
        if mid_adj == 1.0 {
            return;
        }
        let [x0, x1, x2, x3, x4, x5] = self.mid_x[ch];
        let [y0, y1, y2, y3, y4, _] = self.mid_y[ch];
        let y5 = self.mid_y[ch][5];
        let [m0, m1, m2, m3, m4, m5] = self.mid_m[ch];
        let seg = |t: f32, xa: f32, xb: f32, ma: f32, mb: f32, ya: f32| {
            let tt = (t - xa) / (xb - xa);
            tt * (xb - xa) * (tt * 0.5 * (mb - ma) + ma) + ya
        };
        let scalar = ch != M;
        Self::for_channel(out, ch, |t| {
            let fl = seg(t, x0, x1, m0, m1, y0);
            let fm = seg(t, x1, x2, m1, m2, y1);
            let fr = seg(t, x2, x3, m2, m3, y2);
            let fr2 = seg(t, x3, x4, m3, m4, y3);
            let fr3 = seg(t, x4, x5, m4, m5, y4);
            if scalar {
                let mut res = if t < x1 { fl } else { fm };
                if t > x2 {
                    res = fr;
                }
                if t > x3 {
                    res = fr2;
                }
                if t > x4 {
                    res = fr3;
                }
                if t < x0 {
                    res = y0 + (t - x0) * m0;
                }
                if t > x5 {
                    res = y5 + (t - x5) * m5;
                }
                res
            } else {
                let mut res = on_limit(t, x1, fl, fm);
                res = on_limit(t, x2, res, fr);
                res = on_limit(t, x3, res, fr2);
                res = on_limit(t, x4, res, fr3);
                res = on_limit(t, x0, (t - x0) * m0 + y0, res);
                on_limit(t, x5, res, (t - x5) * m5 + y5)
            }
        });
    }

    fn highlight_shadow(&self, ch: usize, is_shadow: bool, out: &mut [f32; 3]) {
        let mut val = if is_shadow {
            self.shadows.channel(ch)
        } else {
            self.highlights.channel(ch)
        };
        if !is_shadow {
            val = 2.0 - val;
        }
        if val == 1.0 {
            return;
        }
        let z = usize::from(is_shadow);
        let [x0, x1, x2] = self.hs_x[z][ch];
        let [y0, y1, y2] = self.hs_y[z][ch];
        let [m0, m2] = self.hs_m[z][ch];
        if val < 1.0 {
            // ComputeHSFwd
            Self::for_channel(out, ch, |t| {
                let tl = (t - x0) / (x1 - x0);
                let tr = (t - x1) / (x2 - x1);
                let fl = y0 * (1.0 - tl * tl) + y1 * tl * tl + m0 * (1.0 - tl) * tl * (x1 - x0);
                let fr = y1 * (1.0 - tr) * (1.0 - tr)
                    + y2 * (2.0 - tr) * tr
                    + m2 * (tr - 1.0) * tr * (x2 - x1);
                let mut res = on_limit(t, x1, fl, fr);
                res = on_limit(t, x0, (t - x0) * m0 + y0, res);
                on_limit(t, x2, res, (t - x2) * m2 + y2)
            });
        } else {
            // ComputeHSRev
            Self::for_channel(out, ch, |t| {
                let bl = m0 * (x1 - x0);
                let al = y1 - y0 - m0 * (x1 - x0);
                let cl = y0 - t;
                let discrim_l = libm::sqrtf(bl * bl - 4.0 * al * cl);
                let out_l = (-2.0 * cl) / (discrim_l + bl) * (x1 - x0) + x0;
                let br = 2.0 * y2 - 2.0 * y1 - m2 * (x2 - x1);
                let ar = y1 - y2 + m2 * (x2 - x1);
                let cr = y1 - t;
                let discrim_r = libm::sqrtf(br * br - 4.0 * ar * cr);
                let out_r = (-2.0 * cr) / (discrim_r + br) * (x2 - x1) + x1;
                let mut res = on_limit(t, y1, out_l, out_r);
                res = on_limit(t, y0, (t - y0) / m0 + x0, res);
                on_limit(t, y2, res, (t - y2) / m2 + x2)
            });
        }
    }

    fn white_black(&self, ch: usize, is_black: bool, out: &mut [f32; 3]) {
        let val = if is_black {
            self.blacks.channel(ch)
        } else {
            self.whites.channel(ch)
        };
        let z = usize::from(is_black);
        let [x0, x1] = self.wb_x[z][ch];
        let [y0, y1] = self.wb_y[z][ch];
        let [m0, m1] = self.wb_m[z][ch];
        let gain = self.wb_gain[z][ch];
        let mtest = if !is_black { val } else { 2.0 - val };
        if mtest < 1.0 {
            Self::for_channel(out, ch, |t| {
                let tl = (t - x0) / (x1 - x0);
                let mut res = tl * (x1 - x0) * (tl * 0.5 * (m1 - m0) + m0) + y0;
                res = on_limit(t, x0, y0 + (t - x0) * m0, res);
                on_limit(t, x1, res, y1 + (t - x1) * m1)
            });
        } else if mtest > 1.0 {
            Self::for_channel(out, ch, |t| {
                let t = if !is_black {
                    (t - x0) * gain + x0
                } else {
                    (t - x1) * gain + x1
                };
                let a = 0.5 * (m1 - m0) * (x1 - x0);
                let b = m0 * (x1 - x0);
                let c = y0 - t;
                let discrim = libm::sqrtf(b * b - 4.0 * a * c);
                let tmp = (-2.0 * c) / (discrim + b);
                let mut res = tmp * (x1 - x0) + x0;
                res = on_limit(t, y0, x0 + (t - y0) / m0, res);
                if !is_black {
                    res = (res - x0) / gain + x0;
                    // Quadratic extrapolation for better HDR control.
                    let new_y1 = (x1 - x0) / gain + x0;
                    let xd = x0 + (x1 - x0) * 0.99;
                    let md = 1.0 / (m0 + (xd - x0) * (m1 - m0) / (x1 - x0));
                    let aa = 0.5 * (1.0 / m1 - md) / (x1 - xd);
                    let bb = 1.0 / m1 - 2.0 * aa * x1;
                    let cc = new_y1 - bb * x1 - aa * x1 * x1;
                    let t = (t - x0) / gain + x0;
                    on_limit(t, x1, res, (aa * t + bb) * t + cc)
                } else {
                    res = on_limit(t, y1, res, x1 + (t - y1) / m1);
                    (res - x1) / gain + x1
                }
            });
        }
    }

    fn scontrast(&self, out: &mut [f32; 3]) {
        let contrast = self.scontrast as f32;
        if contrast == 1.0 {
            return;
        }
        let contrast = if contrast > 1.0 {
            1.0 / (1.8125 - 0.8125 * contrast.min(1.99))
        } else {
            0.28125 + 0.71875 * contrast.max(0.01)
        };
        *out = out.map(|t| {
            let mut o = (t - self.pivot) * contrast + self.pivot;
            {
                let [_, x1, x2, _] = self.sc_x[0];
                let [_, y1, y2, _] = self.sc_y[0];
                let [m0, m3] = self.sc_m[0];
                let tr = (t - x1) / (x2 - x1);
                let res = tr * (x2 - x1) * (tr * 0.5 * (m3 - m0) + m0) + y1;
                o = on_limit(t, x1, o, res);
                o = on_limit(t, x2, o, y2 + (t - x2) * m3);
            }
            {
                let [_, x1, x2, _] = self.sc_x[1];
                let [_, y1, _, _] = self.sc_y[1];
                let [m0, m3] = self.sc_m[1];
                let tr = (t - x1) / (x2 - x1);
                let res = tr * (x2 - x1) * (tr * 0.5 * (m3 - m0) + m0) + y1;
                o = on_limit(t, x2, res, o);
                o = on_limit(t, x1, y1 + (t - x1) * m0, o);
            }
            o
        });
    }

    /// Midtones, highlights, whites, shadows, blacks, s-contrast; values capped at half max.
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        if self.bypass {
            return p;
        }
        let mut out = p;
        for ch in 0..4 {
            self.mids(ch, &mut out);
        }
        for ch in 0..4 {
            self.highlight_shadow(ch, false, &mut out);
        }
        for ch in 0..4 {
            self.white_black(ch, false, &mut out);
        }
        for ch in 0..4 {
            self.highlight_shadow(ch, true, &mut out);
        }
        for ch in 0..4 {
            self.white_black(ch, true, &mut out);
        }
        self.scontrast(&mut out);
        out.map(|v| v.min(65504.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_rules_follow_blender() {
        use ViewTransform::*;
        assert!(!look_applies(AgX, "None").unwrap());
        assert!(look_applies(AgX, "AgX - Punchy").unwrap());
        assert!(!look_applies(Standard, "AgX - Punchy").unwrap());
        assert!(!look_applies(AgX, "High Contrast").unwrap());
        assert!(look_applies(Filmic, "High Contrast").unwrap());
        assert!(look_applies(Raw, "High Contrast").unwrap());
        assert!(look_applies(KhronosPbrNeutral, "Low Contrast").unwrap());
        assert!(look_applies(FalseColor, "False Color - Punchy").unwrap());
        assert!(look_applies(Aces13, "ACES 1.3 - Reference Gamut Compression").unwrap());
        assert!(!look_applies(Aces13, "ACES 2.0 - Reference Gamut Compression").unwrap());
        assert!(matches!(
            look_applies(AgX, "Punchy"),
            Err(ColorError::UnknownLook(_))
        ));
        assert_eq!(looks_for(AgX).len(), 10);
        assert_eq!(looks_for(FalseColor).len(), 10);
        assert_eq!(looks_for(Standard).len(), 8);
        assert_eq!(looks_for(Aces20).len(), 2);
        assert_eq!(
            effective_look(Filmic, "Medium Contrast").unwrap(),
            "Medium Contrast"
        );
        assert_eq!(effective_look(AgX, "High Contrast").unwrap(), "None");
        assert_eq!(effective_look(AgX, "AgX - Punchy").unwrap(), "AgX - Punchy");
    }

    #[test]
    fn cdl_no_clamp_passes_negatives_through_the_power() {
        let cdl = Cdl::new(&[1., 1., 1., 0., 0., 0., 2., 2., 2., 1.], false).unwrap();
        let o = cdl.apply([0.5, -0.5, f32::NAN]);
        assert!((o[0] - 0.25).abs() < 1e-4 && (o[1] + 0.5).abs() < 1e-6 && o[2].abs() < 1e-6);
        let asc = Cdl::new(&[2., 2., 2., 0., 0., 0., 1., 1., 1., 1.], true).unwrap();
        let o = asc.apply([0.75, -0.5, 0.25]);
        assert!(
            (o[0] - 1.0).abs() < 1e-4 && o[1] == 0.0 && (o[2] - 0.5).abs() < 1e-4,
            "{o:?}"
        );
        // Saturation 0 gives luma.
        let grey = Cdl::new(&[1., 1., 1., 0., 0., 0., 1., 1., 1., 0.], false).unwrap();
        let o = grey.apply([1.0, 0.0, 0.0]);
        assert!(o.iter().all(|v| (v - 0.2126).abs() < 1e-4));
    }

    #[test]
    fn grading_primary_log_contrast_pivots_at_0_4() {
        // AgX - High Contrast: contrast 1.4, saturation 0.95, pivot -0.2 (→ 0.4).
        let v = [
            0.,
            0.,
            0.,
            0.,
            1.4,
            1.4,
            1.4,
            1.,
            1.,
            1.,
            1.,
            1.,
            0.95,
            -0.2,
            0.,
            1.,
            -f64::MAX,
            f64::MAX,
        ];
        let g = GradingPrimaryLog::new(&v);
        let o = g.apply([0.4; 3]);
        assert!(o.iter().all(|x| (x - 0.4).abs() < 1e-6));
        let o = g.apply([0.5; 3]);
        assert!(o.iter().all(|x| (x - 0.54).abs() < 1e-6));
        // Identity values bypass.
        let id = [
            0.,
            0.,
            0.,
            0.,
            1.,
            1.,
            1.,
            1.,
            1.,
            1.,
            1.,
            1.,
            1.,
            -0.2,
            0.,
            1.,
            -f64::MAX,
            f64::MAX,
        ];
        assert_eq!(
            GradingPrimaryLog::new(&id).apply([3.0, -1.0, 0.2]),
            [3.0, -1.0, 0.2]
        );
    }

    #[test]
    fn grading_tone_shadows_darken_below_the_pivot_only() {
        // AgX - Punchy's GradingTone: shadows 0.2 (master 0.35), start 0.4, pivot 0.1.
        let v = [
            1., 1., 1., 1., 0.4, 0.4, 0.2, 0.2, 0.2, 0.35, 0.4, 0.1, 1., 1., 1., 1., 0.4, 0.6, 1.,
            1., 1., 1., 0.3, 1., 1., 1., 1., 1., 0.4, 0.5, 1.,
        ];
        let t = GradingToneLog::new(&v);
        assert!(!t.bypass);
        // Above the shadow zone (start 0.41) nothing changes.
        for x in [0.45f32, 0.6, 0.9, 1.2] {
            let o = t.apply([x; 3]);
            assert!(o.iter().all(|v| (v - x).abs() < 1e-6), "{x} -> {o:?}");
        }
        // Inside it values go down, monotonically.
        let mut last = -1.0;
        for i in 0..40 {
            let x = 0.1 + i as f32 * 0.0075;
            let o = t.apply([x; 3])[0];
            assert!(o <= x + 1e-6 && o >= last, "{x} -> {o}");
            last = o;
        }
        // Identity values bypass.
        let mut id = v;
        for z in 0..5 {
            for c in 0..4 {
                id[z * 6 + c] = 1.0;
            }
        }
        assert!(GradingToneLog::new(&id).bypass);
    }

    #[test]
    fn grading_tone_other_zones_are_continuous_and_monotonic() {
        // Every zone and the s-contrast moved at once: the curve stays monotonic.
        let v = [
            1.3, 1.0, 0.8, 1.2, 0.4, 0.4, 0.7, 1.2, 1.0, 0.9, 0.5, 0.1, 1.4, 0.8, 1.0, 1.2, 0.4,
            0.6, 0.6, 1.0, 1.3, 1.1, 0.3, 1.0, 0.8, 1.2, 1.0, 0.9, 0.4, 0.5, 1.3,
        ];
        let t = GradingToneLog::new(&v);
        let mut last = [-1.0f32; 3];
        for i in 0..=400 {
            let x = -0.1 + i as f32 * 0.003;
            let o = t.apply([x; 3]);
            for c in 0..3 {
                assert!(o[c].is_finite(), "{x}: {o:?}");
                assert!(o[c] >= last[c] - 1e-4, "{x}: {o:?} after {last:?}");
            }
            last = o;
        }
    }
}
