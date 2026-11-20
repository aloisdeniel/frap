// SPDX-License-Identifier: GPL-3.0-or-later
//! The ACES 2.0 output transform (OCIO `FIXED_FUNCTION_ACES_OUTPUT_TRANSFORM_20`, forward).
//!
//! Ported from OCIO 2.5 `src/OpenColorIO/ops/fixedfunction/ACES2/{Transform.cpp,Common.h,
//! ColorLib.h,MatrixLib.h}`, `FixedFunctionOpCPU.cpp` (`Renderer_ACES_OutputTransform20`) and
//! `transforms/builtins/ColorMatrixHelpers.cpp` (BSD-3-Clause, Copyright Contributors to the
//! OpenColorIO Project). Computation is in f32 like OCIO's; the primaries matrices are built in
//! f64 and rounded, as OCIO's `MatrixOpData` does. Input is ACES2065-1 (AP0) scene linear,
//! output is display-linear RGB in the limiting primaries (peak 1 = `peak_luminance / 100`).

type F3 = [f32; 3];
type M33 = [f32; 9];

/// `ACES2::PI` (`3.14159265358979f`, the same f32 as `f32::consts::PI`).
const PI: f32 = std::f32::consts::PI;
const HUE_LIMIT: f32 = 360.0;

const REFERENCE_LUMINANCE: f32 = 100.0;
const L_A: f32 = 100.0;
const Y_B: f32 = 20.0;
const SURROUND: F3 = [0.9, 0.59, 0.9];
const J_SCALE: f32 = 100.0;
const CAM_NL_OFFSET: f32 = 0.2713 * 100.0;
const CAM_NL_SCALE: f32 = 4.0 * 100.0;

const CHROMA_COMPRESS: f32 = 2.4;
const CHROMA_COMPRESS_FACT: f32 = 3.3;
const CHROMA_EXPAND: f32 = 1.3;
const CHROMA_EXPAND_FACT: f32 = 0.69;
const CHROMA_EXPAND_THR: f32 = 0.5;

const SMOOTH_CUSPS: f32 = 0.12;
const SMOOTH_M: f32 = 0.27;
const CUSP_MID_BLEND: f32 = 1.3;
const FOCUS_GAIN_BLEND: f32 = 0.3;
const FOCUS_DISTANCE: f32 = 1.35;
const FOCUS_DISTANCE_SCALING: f32 = 1.75;
const COMPRESSION_THRESHOLD: f32 = 0.75;

const GAMMA_MINIMUM: f32 = 0.0;
const GAMMA_MAXIMUM: f32 = 5.0;
const GAMMA_SEARCH_STEP: f32 = 0.4;
const GAMMA_ACCURACY: f32 = 1e-5;

const CUSP_CORNER_COUNT: usize = 6;
const TOTAL_CORNER_COUNT: usize = CUSP_CORNER_COUNT + 2;
const MAX_SORTED_CORNERS: usize = 2 * CUSP_CORNER_COUNT;
const REACH_CUSP_TOLERANCE: f32 = 1e-3;
const DISPLAY_CUSP_TOLERANCE: f32 = 1e-7;

// `TableBase`: 360 nominal entries plus a wrap entry at each end.
const NOMINAL_SIZE: usize = 360;
const TOTAL_SIZE: usize = NOMINAL_SIZE + 2;
const LOWER_WRAP: usize = 0;
const FIRST_NOMINAL: usize = 1;
const UPPER_WRAP: usize = 1 + NOMINAL_SIZE;
const LAST_NOMINAL: usize = UPPER_WRAP - 1;

/// Chromaticities `[red, green, blue, white]` as `(x, y)`.
type Primaries = [(f64, f64); 4];

const AP0: Primaries = [
    (0.7347, 0.2653),
    (0.0000, 1.0000),
    (0.0001, -0.0770),
    (0.32168, 0.33767),
];
const AP1: Primaries = [
    (0.713, 0.293),
    (0.165, 0.830),
    (0.128, 0.044),
    (0.32168, 0.33767),
];
const CAM16: Primaries = [
    (0.8336, 0.1735),
    (2.3854, -1.4659),
    (0.087, -0.125),
    (0.333, 0.333),
];

#[inline]
fn lerpf(a: f32, b: f32, z: f32) -> f32 {
    (b - a) * z + a
}

#[inline]
fn lerp3(a: F3, b: F3, t: f32) -> F3 {
    [
        lerpf(a[0], b[0], t),
        lerpf(a[1], b[1], t),
        lerpf(a[2], b[2], t),
    ]
}

#[inline]
fn mul_m_v(v: F3, m: &M33) -> F3 {
    [
        v[0] * m[0] + v[1] * m[1] + v[2] * m[2],
        v[0] * m[3] + v[1] * m[4] + v[2] * m[5],
        v[0] * m[6] + v[1] * m[7] + v[2] * m[8],
    ]
}

fn mul_m_m(a: &M33, b: &M33) -> M33 {
    let mut o = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            o[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
        }
    }
    o
}

fn diag(s: F3) -> M33 {
    [s[0], 0.0, 0.0, 0.0, s[1], 0.0, 0.0, 0.0, s[2]]
}

fn invert_f64(m: [f64; 9]) -> [f64; 9] {
    let [a, b, c, d, e, f, g, h, i] = m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    [
        (e * i - f * h) / det,
        (c * h - b * i) / det,
        (b * f - c * e) / det,
        (f * g - d * i) / det,
        (a * i - c * g) / det,
        (c * d - a * f) / det,
        (d * h - e * g) / det,
        (b * g - a * h) / det,
        (a * e - b * d) / det,
    ]
}

fn to_f32(m: [f64; 9]) -> M33 {
    m.map(|v| v as f32)
}

/// `invert_f33`: the inverse computed in f64 from the f32 matrix.
fn invert_f33(m: &M33) -> M33 {
    to_f32(invert_f64(m.map(f64::from)))
}

/// `rgb2xyz_from_xy` (white Y = 1), f64.
fn rgb2xyz(p: &Primaries) -> [f64; 9] {
    let m = [
        p[0].0,
        p[1].0,
        p[2].0,
        p[0].1,
        p[1].1,
        p[2].1,
        1.0 - p[0].0 - p[0].1,
        1.0 - p[1].0 - p[1].1,
        1.0 - p[2].0 - p[2].1,
    ];
    let inv = invert_f64(m);
    let (wx, wy) = p[3];
    let white = [wx / wy, 1.0, (1.0 - wx - wy) / wy];
    let mut out = [0.0; 9];
    for i in 0..3 {
        let gain = white[0] * inv[i * 3] + white[1] * inv[i * 3 + 1] + white[2] * inv[i * 3 + 2];
        for j in 0..3 {
            out[j * 3 + i] = gain * m[j * 3 + i];
        }
    }
    out
}

/// `RGBtoXYZ_f33` (to XYZ with illuminant E, no adaptation: the plain primaries matrix).
fn rgb_to_xyz_f33(p: &Primaries) -> M33 {
    to_f32(rgb2xyz(p))
}

/// `XYZtoRGB_f33`.
fn xyz_to_rgb_f33(p: &Primaries) -> M33 {
    to_f32(invert_f64(rgb2xyz(p)))
}

// --- CAM ------------------------------------------------------------------------------------

#[inline]
fn compress_fwd_abs(rc: f32) -> f32 {
    let f_l_y = libm::powf(rc, 0.42);
    f_l_y / (CAM_NL_OFFSET + f_l_y)
}

#[inline]
fn compress_inv_abs(ra: f32) -> f32 {
    let ra_lim = ra.min(0.99);
    let f_l_y = (CAM_NL_OFFSET * ra_lim) / (1.0 - ra_lim);
    libm::powf(f_l_y, 1.0 / 0.42)
}

#[inline]
fn compress_fwd(v: f32) -> f32 {
    libm::copysignf(compress_fwd_abs(v.abs()), v)
}

#[inline]
fn compress_inv(v: f32) -> f32 {
    libm::copysignf(compress_inv_abs(v.abs()), v)
}

#[inline]
fn achromatic_n_to_j(a: f32, cz: f32) -> f32 {
    J_SCALE * libm::powf(a, cz)
}

#[inline]
fn j_to_achromatic_n(j: f32, inv_cz: f32) -> f32 {
    libm::powf(j * (1.0 / J_SCALE), inv_cz)
}

fn model_gamma() -> f32 {
    SURROUND[1] * (1.48 + libm::sqrtf(Y_B / REFERENCE_LUMINANCE))
}

#[inline]
fn wrap_hue(y: f32) -> f32 {
    if y < 0.0 { y + HUE_LIMIT } else { y }
}

#[inline]
fn to_radians(v: f32) -> f32 {
    PI * v / 180.0
}

#[derive(Clone, Debug, PartialEq)]
struct JMhParams {
    rgb_to_cam16_c: M33,
    cam16_c_to_rgb: M33,
    cone_response_to_aab: M33,
    aab_to_cone_response: M33,
    f_l_n: f32,
    cz: f32,
    inv_cz: f32,
    a_w_j: f32,
    inv_a_w_j: f32,
}

impl JMhParams {
    /// `init_JMhParams`.
    fn new(prims: &Primaries) -> JMhParams {
        let base: M33 = [
            2.0,
            1.0,
            1.0 / 20.0,
            1.0,
            -12.0 / 11.0,
            1.0 / 11.0,
            1.0 / 9.0,
            1.0 / 9.0,
            -2.0 / 9.0,
        ];
        let matrix_16 = xyz_to_rgb_f33(&CAM16);
        let rgb_to_xyz = rgb_to_xyz_f33(prims);
        let xyz_w = mul_m_v([REFERENCE_LUMINANCE; 3], &rgb_to_xyz);
        let y_w = xyz_w[1];
        let rgb_w = mul_m_v(xyz_w, &matrix_16);

        let k = 1.0 / (5.0 * L_A + 1.0);
        let k4 = k * k * k * k;
        let f_l = 0.2 * k4 * (5.0 * L_A)
            + 0.1 * libm::powf(1.0 - k4, 2.0) * libm::powf(5.0 * L_A, 1.0 / 3.0);
        let f_l_n = f_l / REFERENCE_LUMINANCE;
        let cz = model_gamma();
        let inv_cz = 1.0 / cz;

        let d_rgb = [0, 1, 2].map(|i| f_l_n * y_w / rgb_w[i]);
        let rgb_wc = [0, 1, 2].map(|i| d_rgb[i] * rgb_w[i]);
        let rgb_aw = rgb_wc.map(compress_fwd);

        let cone_response_to_aab = mul_m_m(&diag([CAM_NL_SCALE; 3]), &base);
        let a_w = cone_response_to_aab[0] * rgb_aw[0]
            + cone_response_to_aab[1] * rgb_aw[1]
            + cone_response_to_aab[2] * rgb_aw[2];
        let a_w_j = compress_fwd_abs(f_l);

        let rgb_to_rgb = mul_m_m(&xyz_to_rgb_f33(&CAM16), &rgb_to_xyz);
        let rgb_to_cam16 = mul_m_m(&rgb_to_rgb, &diag([REFERENCE_LUMINANCE; 3]));
        let rgb_to_cam16_c = mul_m_m(&diag(d_rgb), &rgb_to_cam16);
        let cam16_c_to_rgb = invert_f33(&rgb_to_cam16_c);

        let s = 43.0 * SURROUND[2];
        let c = &cone_response_to_aab;
        let cone_to_aab: M33 = [
            c[0] / a_w,
            c[1] / a_w,
            c[2] / a_w,
            c[3] * s,
            c[4] * s,
            c[5] * s,
            c[6] * s,
            c[7] * s,
            c[8] * s,
        ];
        JMhParams {
            rgb_to_cam16_c,
            cam16_c_to_rgb,
            cone_response_to_aab: cone_to_aab,
            aab_to_cone_response: invert_f33(&cone_to_aab),
            f_l_n,
            cz,
            inv_cz,
            a_w_j,
            inv_a_w_j: 1.0 / a_w_j,
        }
    }

    fn rgb_to_aab(&self, rgb: F3) -> F3 {
        let m = mul_m_v(rgb, &self.rgb_to_cam16_c);
        mul_m_v(m.map(compress_fwd), &self.cone_response_to_aab)
    }

    fn aab_to_jmh(&self, aab: F3) -> F3 {
        if aab[0] <= 0.0 {
            return [0.0; 3];
        }
        let j = achromatic_n_to_j(aab[0], self.cz);
        let m = libm::sqrtf(aab[1] * aab[1] + aab[2] * aab[2]);
        let h = wrap_hue(180.0 * libm::atan2f(aab[2], aab[1]) / PI);
        [j, m, h]
    }

    fn rgb_to_jmh(&self, rgb: F3) -> F3 {
        self.aab_to_jmh(self.rgb_to_aab(rgb))
    }

    fn jmh_to_aab_cs(&self, jmh: F3, cos_hr: f32, sin_hr: f32) -> F3 {
        [
            j_to_achromatic_n(jmh[0], self.inv_cz),
            jmh[1] * cos_hr,
            jmh[1] * sin_hr,
        ]
    }

    fn aab_to_rgb(&self, aab: F3) -> F3 {
        let a = mul_m_v(aab, &self.aab_to_cone_response);
        mul_m_v(a.map(compress_inv), &self.cam16_c_to_rgb)
    }

    fn jmh_to_rgb(&self, jmh: F3) -> F3 {
        let h = to_radians(jmh[2]);
        self.aab_to_rgb(self.jmh_to_aab_cs(jmh, libm::cosf(h), libm::sinf(h)))
    }

    fn a_to_y(&self, a: f32) -> f32 {
        compress_inv_abs(self.a_w_j * a) / self.f_l_n
    }

    fn y_to_j_abs(&self, abs_y: f32) -> f32 {
        let ra = compress_fwd_abs(abs_y * self.f_l_n);
        achromatic_n_to_j(ra * self.inv_a_w_j, self.cz)
    }

    fn y_to_j(&self, y: f32) -> f32 {
        libm::copysignf(self.y_to_j_abs(y.abs()), y)
    }
}

// --- tone scale and chroma compression ------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct ToneScale {
    n: f32,
    n_r: f32,
    g: f32,
    t_1: f32,
    c_t: f32,
    s_2: f32,
    m_2: f32,
    forward_limit: f32,
    log_peak: f32,
}

impl ToneScale {
    /// `init_ToneScaleParams`.
    fn new(peak: f32) -> ToneScale {
        let n = peak;
        let n_r = 100.0f32;
        let g = 1.15f32;
        let c = 0.18f32;
        let c_d = 10.013f32;
        let w_g = 0.14f32;
        let t_1 = 0.04f32;
        let r_hit_min = 128.0f32;
        let r_hit_max = 896.0f32;
        let r_hit = r_hit_min
            + (r_hit_max - r_hit_min) * (libm::logf(n / n_r) / libm::logf(10000.0 / 100.0));
        let m_0 = n / n_r;
        let m_1 = 0.5 * (m_0 + libm::sqrtf(m_0 * (m_0 + 4.0 * t_1)));
        let u = libm::powf((r_hit / m_1) / ((r_hit / m_1) + 1.0), g);
        let m = m_1 / u;
        let w_i = libm::logf(n / 100.0) / libm::logf(2.0);
        let c_t = c_d / n_r * (1.0 + w_i * w_g);
        let g_ip = 0.5 * (c_t + libm::sqrtf(c_t * (c_t + 4.0 * t_1)));
        let g_ipp2 = -(m_1 * libm::powf(g_ip / m, 1.0 / g)) / (libm::powf(g_ip / m, 1.0 / g) - 1.0);
        let w_2 = c / g_ipp2;
        let s_2 = w_2 * m_1 * REFERENCE_LUMINANCE;
        let u_2 = libm::powf((r_hit / m_1) / ((r_hit / m_1) + w_2), g);
        let m_2 = m_1 / u_2;
        ToneScale {
            n,
            n_r,
            g,
            t_1,
            c_t,
            s_2,
            m_2,
            forward_limit: 8.0 * r_hit,
            log_peak: libm::log10f(n / n_r),
        }
    }

    /// `aces_tonescale<false>`.
    fn apply(&self, y_in: f32) -> f32 {
        let f = self.m_2 * libm::powf(y_in / (y_in + self.s_2), self.g);
        (f * f / (f + self.t_1)).max(0.0) * self.n_r
    }
}

#[inline]
fn toe_fwd(x: f32, limit: f32, k1_in: f32, k2_in: f32) -> f32 {
    if x > limit {
        return x;
    }
    let k2 = k2_in.max(0.001);
    let k1 = libm::sqrtf(k1_in * k1_in + k2 * k2);
    let k3 = (limit + k1) / (limit + k2);
    let minus_b = k3 * x - k1;
    let minus_ac = k2 * k3 * x;
    0.5 * (minus_b + libm::sqrtf(minus_b * minus_b + 4.0 * minus_ac))
}

#[derive(Clone, Debug, PartialEq)]
struct ChromaCompress {
    sat: f32,
    sat_thr: f32,
    compr: f32,
    scale: f32,
}

fn chroma_compress_norm(cos_hr1: f32, sin_hr1: f32, scale: f32) -> f32 {
    let cos_hr2 = 2.0 * cos_hr1 * cos_hr1 - 1.0;
    let sin_hr2 = 2.0 * cos_hr1 * sin_hr1;
    let cos_hr3 = 4.0 * cos_hr1 * cos_hr1 * cos_hr1 - 3.0 * cos_hr1;
    let sin_hr3 = 3.0 * sin_hr1 - 4.0 * sin_hr1 * sin_hr1 * sin_hr1;
    let m = 11.34072 * cos_hr1
        + 16.46899 * cos_hr2
        + 7.88380 * cos_hr3
        + 14.66441 * sin_hr1
        + -6.37224 * sin_hr2
        + 9.19364 * sin_hr3
        + 77.12896;
    m * scale
}

// --- gamut compression ----------------------------------------------------------------------

/// `Table1D` / `Table3D` values: the reach-M table, hue table and cusp table.
type Table1 = [f32; TOTAL_SIZE];
type Table3 = [F3; TOTAL_SIZE];

#[derive(Clone, Debug, PartialEq)]
struct Shared {
    limit_j_max: f32,
    model_gamma_inv: f32,
    reach_m_table: Box<Table1>,
}

struct Resolved {
    limit_j_max: f32,
    model_gamma_inv: f32,
    reach_max_m: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct GamutCompress {
    mid_j: f32,
    focus_dist: f32,
    lower_hull_gamma_inv: f32,
    search_range: [i32; 2],
    hue_table: Box<Table1>,
    cusp_table: Box<Table3>,
}

fn reach_m_from_table(h: f32, t: &Table1) -> f32 {
    let base = h as usize;
    let frac = h - base as f32;
    let i_lo = (base + FIRST_NOMINAL).min(TOTAL_SIZE - 2);
    lerpf(t[i_lo], t[i_lo + 1], frac)
}

fn any_below_zero(rgb: F3) -> bool {
    rgb[0] < 0.0 || rgb[1] < 0.0 || rgb[2] < 0.0
}

/// `make_reach_m_table`.
fn make_reach_m_table(params: &JMhParams, limit_j_max: f32) -> Box<Table1> {
    let mut t = Box::new([0.0f32; TOTAL_SIZE]);
    for i in 0..NOMINAL_SIZE {
        let hue = i as f32;
        const SEARCH_RANGE: f32 = 50.0;
        const SEARCH_MAXIMUM: f32 = 1300.0;
        let mut low = 0.0f32;
        let mut high = low + SEARCH_RANGE;
        let mut outside = false;
        while !outside && high < SEARCH_MAXIMUM {
            outside = any_below_zero(params.jmh_to_rgb([limit_j_max, high, hue]));
            if !outside {
                low = high;
                high += SEARCH_RANGE;
            }
        }
        while high - low > 1e-2 {
            let sample = (high + low) / 2.0;
            if any_below_zero(params.jmh_to_rgb([limit_j_max, sample, hue])) {
                high = sample;
            } else {
                low = sample;
            }
        }
        t[i + 1] = high;
    }
    t[LOWER_WRAP] = t[LAST_NOMINAL];
    t[UPPER_WRAP] = t[FIRST_NOMINAL];
    t
}

/// `generate_unit_cube_cusp_corners`: R, Y, G, C, B, M.
fn cube_corner(corner: usize) -> F3 {
    [
        f32::from(u8::from((corner + 1) % CUSP_CORNER_COUNT < 3)),
        f32::from(u8::from((corner + 5) % CUSP_CORNER_COUNT < 3)),
        f32::from(u8::from((corner + 3) % CUSP_CORNER_COUNT < 3)),
    ]
}

/// Rotates six corners so the lowest hue sits at `[1]`, and wraps both ends.
fn cycle_corners<T: Copy>(
    temp: &[T; CUSP_CORNER_COUNT],
    min_index: usize,
) -> [T; TOTAL_CORNER_COUNT] {
    let mut out = [temp[0]; TOTAL_CORNER_COUNT];
    for i in 0..CUSP_CORNER_COUNT {
        out[i + 1] = temp[(i + min_index) % CUSP_CORNER_COUNT];
    }
    out[0] = out[CUSP_CORNER_COUNT];
    out[CUSP_CORNER_COUNT + 1] = out[1];
    out
}

/// `build_limiting_cusp_corners_tables`.
fn limiting_corners(
    params: &JMhParams,
    peak: f32,
) -> ([F3; TOTAL_CORNER_COUNT], [F3; TOTAL_CORNER_COUNT]) {
    let mut rgb = [[0.0f32; 3]; CUSP_CORNER_COUNT];
    let mut jmh = [[0.0f32; 3]; CUSP_CORNER_COUNT];
    let mut min_index = 0;
    for i in 0..CUSP_CORNER_COUNT {
        rgb[i] = cube_corner(i).map(|v| v * (peak / REFERENCE_LUMINANCE));
        jmh[i] = params.rgb_to_jmh(rgb[i]);
        if jmh[i][2] < jmh[min_index][2] {
            min_index = i;
        }
    }
    let rgb_c = cycle_corners(&rgb, min_index);
    let mut jmh_c = cycle_corners(&jmh, min_index);
    jmh_c[0][2] -= HUE_LIMIT;
    jmh_c[CUSP_CORNER_COUNT + 1][2] += HUE_LIMIT;
    (rgb_c, jmh_c)
}

/// `find_reach_corners_table`.
fn reach_corners(
    params: &JMhParams,
    limit_j: f32,
    maximum_source: f32,
) -> [F3; TOTAL_CORNER_COUNT] {
    let limit_a = j_to_achromatic_n(limit_j, params.inv_cz);
    let mut jmh = [[0.0f32; 3]; CUSP_CORNER_COUNT];
    let mut min_index = 0;
    for i in 0..CUSP_CORNER_COUNT {
        let v = cube_corner(i);
        let mut lower = 0.0f32;
        let mut upper = maximum_source;
        while upper - lower > REACH_CUSP_TOLERANCE {
            let test = (lower + upper) / 2.0;
            let a = params.rgb_to_aab(v.map(|c| c * test))[0];
            if a < limit_a {
                lower = test;
            } else {
                upper = test;
            }
            if a == limit_a {
                break;
            }
        }
        jmh[i] = params.rgb_to_jmh(v.map(|c| c * upper));
        if jmh[i][2] < jmh[min_index][2] {
            min_index = i;
        }
    }
    let mut out = cycle_corners(&jmh, min_index);
    out[0][2] -= HUE_LIMIT;
    out[CUSP_CORNER_COUNT + 1][2] += HUE_LIMIT;
    out
}

/// `extract_sorted_cube_hues`: merge of the two sorted corner hue lists, unique values.
fn sorted_cube_hues(
    reach: &[F3; TOTAL_CORNER_COUNT],
    display: &[F3; TOTAL_CORNER_COUNT],
) -> ([f32; MAX_SORTED_CORNERS], usize) {
    let mut sorted = [0.0f32; MAX_SORTED_CORNERS];
    let (mut idx, mut ri, mut di) = (0usize, 1usize, 1usize);
    while ri < CUSP_CORNER_COUNT + 1 || di < CUSP_CORNER_COUNT + 1 {
        let rh = reach[ri][2];
        let dh = display[di][2];
        if rh == dh {
            sorted[idx] = rh;
            ri += 1;
            di += 1;
        } else if rh < dh {
            sorted[idx] = rh;
            ri += 1;
        } else {
            sorted[idx] = dh;
            di += 1;
        }
        idx += 1;
    }
    (sorted, idx)
}

fn hue_interval(samples: usize, lower: f32, upper: f32, table: &mut Table1, base: usize) {
    let delta = (upper - lower) / samples as f32;
    for i in 0..samples {
        table[base + i] = lower + i as f32 * delta;
    }
}

/// `build_hue_table`.
fn build_hue_table(sorted: &[f32; MAX_SORTED_CORNERS], unique: usize) -> Box<Table1> {
    let mut table = Box::new([0.0f32; TOTAL_SIZE]);
    let ideal_spacing = NOMINAL_SIZE as f32 / HUE_LIMIT;
    let mut samples_count = [0usize; 2 * CUSP_CORNER_COUNT + 2];
    let mut last_idx = usize::MAX;
    let mut min_index = if sorted[0] == 0.0 { 0 } else { 1 };
    for hue_idx in 0..unique {
        let rounded = libm::roundf(sorted[hue_idx] * ideal_spacing).max(0.0) as usize;
        let mut nominal = rounded.max(min_index).min(NOMINAL_SIZE - 1);
        if last_idx == nominal {
            if hue_idx > 1
                && samples_count[hue_idx - 2] != samples_count[hue_idx - 1].wrapping_sub(1)
            {
                samples_count[hue_idx - 1] -= 1;
            } else {
                nominal += 1;
            }
        }
        samples_count[hue_idx] = nominal.min(NOMINAL_SIZE - 1);
        last_idx = nominal;
        min_index = nominal;
    }
    let mut total = 0usize;
    hue_interval(samples_count[0], 0.0, sorted[0], &mut table, total + 1);
    total += samples_count[0];
    for i in 1..unique {
        let samples = samples_count[i] - samples_count[i - 1];
        hue_interval(samples, sorted[i - 1], sorted[i], &mut table, total + 1);
        total += samples;
    }
    hue_interval(
        NOMINAL_SIZE - total,
        sorted[unique - 1],
        HUE_LIMIT,
        &mut table,
        total + 1,
    );
    table[LOWER_WRAP] = table[LAST_NOMINAL] - HUE_LIMIT;
    table[UPPER_WRAP] = table[FIRST_NOMINAL] + HUE_LIMIT;
    table
}

/// `find_display_cusp_for_hue`: binary search along the gamut-cube edge between two corners.
fn display_cusp_for_hue(
    hue: f32,
    rgb_corners: &[F3; TOTAL_CORNER_COUNT],
    jmh_corners: &[F3; TOTAL_CORNER_COUNT],
    params: &JMhParams,
    previous: &mut [f32; 2],
) -> [f32; 2] {
    let mut upper_corner = 1;
    for (i, c) in jmh_corners.iter().enumerate().skip(1) {
        if c[2] > hue {
            upper_corner = i;
            break;
        }
    }
    let lower_corner = upper_corner - 1;
    if jmh_corners[lower_corner][2] == hue {
        return [jmh_corners[lower_corner][0], jmh_corners[lower_corner][1]];
    }
    let (cusp_lower, cusp_upper) = (rgb_corners[lower_corner], rgb_corners[upper_corner]);
    let mut lower_t = if upper_corner as f32 == previous[0] {
        previous[1]
    } else {
        0.0
    };
    let mut upper_t = 1.0f32;
    while upper_t - lower_t > DISPLAY_CUSP_TOLERANCE {
        let t = (lower_t + upper_t) / 2.0;
        let jmh = params.rgb_to_jmh(lerp3(cusp_lower, cusp_upper, t));
        if jmh[2] < jmh_corners[lower_corner][2] {
            upper_t = t;
        } else if jmh[2] >= jmh_corners[upper_corner][2] {
            lower_t = t;
        } else if jmh[2] > hue {
            upper_t = t;
        } else {
            lower_t = t;
        }
    }
    let t = (lower_t + upper_t) / 2.0;
    let jmh = params.rgb_to_jmh(lerp3(cusp_lower, cusp_upper, t));
    *previous = [upper_corner as f32, t];
    [jmh[0], jmh[1]]
}

fn get_focus_gain(j: f32, analytical_threshold: f32, limit_j_max: f32, focus_dist: f32) -> f32 {
    let mut gain = limit_j_max * focus_dist;
    if j > analytical_threshold {
        let adj =
            libm::log10f((limit_j_max - analytical_threshold) / (limit_j_max - j).max(0.0001));
        gain *= adj * adj + 1.0;
    }
    gain
}

fn solve_j_intersect(j: f32, m: f32, focus_j: f32, max_j: f32, slope_gain: f32) -> f32 {
    let m_scaled = m / slope_gain;
    let a = m_scaled / focus_j;
    if j < focus_j {
        let b = 1.0 - m_scaled;
        let c = -j;
        let root = libm::sqrtf(b * b - 4.0 * a * c);
        -2.0 * c / (b + root)
    } else {
        let b = -(1.0 + m_scaled + max_j * a);
        let c = max_j * m_scaled + j;
        let root = libm::sqrtf(b * b - 4.0 * a * c);
        -2.0 * c / (b - root)
    }
}

fn smin_scaled(a: f32, b: f32, scale_reference: f32) -> f32 {
    let s = SMOOTH_CUSPS * scale_reference;
    let h = (s - (a - b).abs()).max(0.0) / s;
    a.min(b) - h * h * h * s * (1.0 / 6.0)
}

fn compression_vector_slope(
    intersect_j: f32,
    focus_j: f32,
    limit_j_max: f32,
    slope_gain: f32,
) -> f32 {
    let direction_scaler = if intersect_j < focus_j {
        intersect_j
    } else {
        limit_j_max - intersect_j
    };
    direction_scaler * (intersect_j - focus_j) / (focus_j * slope_gain)
}

fn line_boundary_intersection_m(
    j_axis_intersect: f32,
    slope: f32,
    inv_gamma: f32,
    j_max: f32,
    m_max: f32,
    j_intersection_reference: f32,
) -> f32 {
    let normalised_j = j_axis_intersect / j_intersection_reference;
    let shifted = j_intersection_reference * libm::powf(normalised_j, inv_gamma);
    shifted * m_max / (j_max - slope * m_max)
}

fn gamut_boundary_intersection(
    jm_cusp: [f32; 2],
    j_max: f32,
    gamma_top_inv: f32,
    gamma_bottom_inv: f32,
    j_intersect_source: f32,
    slope: f32,
    j_intersect_cusp: f32,
) -> f32 {
    let lower = line_boundary_intersection_m(
        j_intersect_source,
        slope,
        gamma_bottom_inv,
        jm_cusp[0],
        jm_cusp[1],
        j_intersect_cusp,
    );
    let upper = line_boundary_intersection_m(
        j_max - j_intersect_source,
        -slope,
        gamma_top_inv,
        j_max - jm_cusp[0],
        jm_cusp[1],
        j_max - j_intersect_cusp,
    );
    smin_scaled(lower, upper, jm_cusp[1])
}

fn remap_m(m: f32, gamut_boundary_m: f32, reach_boundary_m: f32) -> f32 {
    let proportion = (gamut_boundary_m / reach_boundary_m).max(COMPRESSION_THRESHOLD);
    let threshold = proportion * gamut_boundary_m;
    if m <= threshold || proportion >= 1.0 {
        return m;
    }
    let m_offset = m - threshold;
    let gamut_offset = gamut_boundary_m - threshold;
    let reach_offset = reach_boundary_m - threshold;
    let scale = reach_offset / ((reach_offset / gamut_offset) - 1.0);
    let nd = m_offset / scale;
    threshold + scale * nd / (1.0 + nd)
}

#[inline]
fn compute_focus_j(cusp_j: f32, mid_j: f32, limit_j_max: f32) -> f32 {
    lerpf(
        cusp_j,
        mid_j,
        (CUSP_MID_BLEND - cusp_j / limit_j_max).min(1.0),
    )
}

struct HueDependent {
    gamma_bottom_inv: f32,
    jm_cusp: [f32; 2],
    gamma_top_inv: f32,
    focus_j: f32,
    analytical_threshold: f32,
}

impl GamutCompress {
    /// `lookup_hue_interval`: the upper index of the hue-table interval containing `h`.
    fn hue_interval(&self, h: f32) -> usize {
        let hues = &self.hue_table;
        let mut i = FIRST_NOMINAL as i32 + h as i32;
        let mut i_lo = (LOWER_WRAP as i32).max(i + self.search_range[0]);
        let mut i_hi = (UPPER_WRAP as i32).min(i + self.search_range[1]);
        while i_lo + 1 < i_hi {
            if h > hues[i.clamp(0, TOTAL_SIZE as i32 - 1) as usize] {
                i_lo = i;
            } else {
                i_hi = i;
            }
            i = (i_lo + i_hi) / 2;
        }
        i_hi.max(1) as usize
    }

    fn hue_dependent(&self, hue: f32, limit_j_max: f32) -> HueDependent {
        let i_hi = self.hue_interval(hue);
        let t =
            (hue - self.hue_table[i_hi - 1]) / (self.hue_table[i_hi] - self.hue_table[i_hi - 1]);
        let cusp = lerp3(self.cusp_table[i_hi - 1], self.cusp_table[i_hi], t);
        HueDependent {
            gamma_bottom_inv: self.lower_hull_gamma_inv,
            jm_cusp: [cusp[0], cusp[1]],
            gamma_top_inv: cusp[2],
            focus_j: compute_focus_j(cusp[0], self.mid_j, limit_j_max),
            analytical_threshold: lerpf(cusp[0], limit_j_max, FOCUS_GAIN_BLEND),
        }
    }

    /// `compressGamut<false>`.
    fn compress(&self, jmh: F3, jx: f32, sr: &Resolved, hdp: &HueDependent) -> F3 {
        let [j, m, h] = jmh;
        let slope_gain = get_focus_gain(
            jx,
            hdp.analytical_threshold,
            sr.limit_j_max,
            self.focus_dist,
        );
        let j_src = solve_j_intersect(j, m, hdp.focus_j, sr.limit_j_max, slope_gain);
        let slope = compression_vector_slope(j_src, hdp.focus_j, sr.limit_j_max, slope_gain);
        let j_cusp = solve_j_intersect(
            hdp.jm_cusp[0],
            hdp.jm_cusp[1],
            hdp.focus_j,
            sr.limit_j_max,
            slope_gain,
        );
        let boundary = gamut_boundary_intersection(
            hdp.jm_cusp,
            sr.limit_j_max,
            hdp.gamma_top_inv,
            hdp.gamma_bottom_inv,
            j_src,
            slope,
            j_cusp,
        );
        if boundary <= 0.0 {
            return [j, 0.0, h];
        }
        let reach = line_boundary_intersection_m(
            j_src,
            slope,
            sr.model_gamma_inv,
            sr.limit_j_max,
            sr.reach_max_m,
            sr.limit_j_max,
        );
        let remapped = remap_m(m, boundary, reach);
        [j_src + remapped * slope, remapped, h]
    }

    /// `gamut_compress_fwd`.
    fn forward(&self, jmh: F3, sr: &Resolved) -> F3 {
        let [j, m, h] = jmh;
        if j <= 0.0 {
            return [0.0, 0.0, h];
        }
        if m <= 0.0 || j > sr.limit_j_max {
            return [j, 0.0, h];
        }
        let hdp = self.hue_dependent(h, sr.limit_j_max);
        self.compress(jmh, j, sr, &hdp)
    }
}

/// `generate_gamma_test_data` + `evaluate_gamma_fit`.
#[allow(clippy::too_many_arguments)]
fn gamma_fits(
    jm_cusp: [f32; 2],
    hue: f32,
    top_gamma_inv: f32,
    peak: f32,
    limit_j_max: f32,
    mid_j: f32,
    focus_dist: f32,
    lower_hull_gamma_inv: f32,
    limit_params: &JMhParams,
) -> bool {
    const TEST_POSITIONS: [f32; 5] = [0.01, 0.1, 0.5, 0.8, 0.99];
    let analytical_threshold = lerpf(jm_cusp[0], limit_j_max, FOCUS_GAIN_BLEND);
    let focus_j = compute_focus_j(jm_cusp[0], mid_j, limit_j_max);
    let luminance_limit = peak / REFERENCE_LUMINANCE;
    for pos in TEST_POSITIONS {
        let test_j = lerpf(jm_cusp[0], limit_j_max, pos);
        let slope_gain = get_focus_gain(test_j, analytical_threshold, limit_j_max, focus_dist);
        let j_src = solve_j_intersect(test_j, jm_cusp[1], focus_j, limit_j_max, slope_gain);
        let slope = compression_vector_slope(j_src, focus_j, limit_j_max, slope_gain);
        let j_cusp = solve_j_intersect(jm_cusp[0], jm_cusp[1], focus_j, limit_j_max, slope_gain);
        let m = gamut_boundary_intersection(
            jm_cusp,
            limit_j_max,
            top_gamma_inv,
            lower_hull_gamma_inv,
            j_src,
            slope,
            j_cusp,
        );
        let j = j_src + slope * m;
        let rgb = limit_params.jmh_to_rgb([j, m, hue]);
        let outside =
            rgb[0] > luminance_limit || rgb[1] > luminance_limit || rgb[2] > luminance_limit;
        if !outside {
            return false;
        }
    }
    true
}

/// The ACES 2.0 output transform, with its tables (built once per pipeline).
#[derive(Clone, Debug, PartialEq)]
pub struct OutputTransform20 {
    p_in: JMhParams,
    p_out: JMhParams,
    tone: ToneScale,
    shared: Shared,
    chroma: ChromaCompress,
    gamut: GamutCompress,
}

impl OutputTransform20 {
    /// From `peak_luminance red_x red_y green_x green_y blue_x blue_y white_x white_y`
    /// (`Renderer_ACES_OutputTransform20` constructor).
    pub fn new(params: &[f64]) -> Result<OutputTransform20, String> {
        if params.len() != 9 {
            return Err("aces_output_transform_20: expected 9 parameters".into());
        }
        let peak = params[0] as f32;
        // OCIO stores the limiting chromaticities as floats before building the matrices.
        let c = |i: usize| f64::from(params[i] as f32);
        let lim: Primaries = [(c(1), c(2)), (c(3), c(4)), (c(5), c(6)), (c(7), c(8))];
        let p_in = JMhParams::new(&AP0);
        let p_out = JMhParams::new(&lim);
        let tone = ToneScale::new(peak);
        let reach = JMhParams::new(&AP1);
        let limit_j_max = p_in.y_to_j(peak);
        let shared = Shared {
            limit_j_max,
            model_gamma_inv: 1.0 / model_gamma(),
            reach_m_table: make_reach_m_table(&reach, limit_j_max),
        };
        let chroma = ChromaCompress {
            compr: CHROMA_COMPRESS + (CHROMA_COMPRESS * CHROMA_COMPRESS_FACT) * tone.log_peak,
            sat: (CHROMA_EXPAND - (CHROMA_EXPAND * CHROMA_EXPAND_FACT) * tone.log_peak).max(0.2),
            sat_thr: CHROMA_EXPAND_THR / tone.n,
            scale: libm::powf(0.03379 * peak, 0.30596) - 0.45135,
        };
        // `init_GamutCompressParams`.
        let mid_j = p_in.y_to_j(tone.c_t * REFERENCE_LUMINANCE);
        let focus_dist = FOCUS_DISTANCE + FOCUS_DISTANCE * FOCUS_DISTANCE_SCALING * tone.log_peak;
        let lower_hull_gamma_inv = 1.0 / (1.14 + 0.07 * tone.log_peak);
        // `make_uniform_hue_gamut_table`.
        let reach_corners = reach_corners(&reach, limit_j_max, tone.forward_limit);
        let (lim_rgb, lim_jmh) = limiting_corners(&p_out, peak);
        let (sorted, unique) = sorted_cube_hues(&reach_corners, &lim_jmh);
        let hue_table = build_hue_table(&sorted, unique);
        let mut cusp_table = Box::new([[0.0f32; 3]; TOTAL_SIZE]);
        let mut previous = [0.0f32; 2];
        for i in FIRST_NOMINAL..UPPER_WRAP {
            let hue = hue_table[i];
            let jm = display_cusp_for_hue(hue, &lim_rgb, &lim_jmh, &p_out, &mut previous);
            cusp_table[i] = [jm[0], jm[1] * (1.0 + SMOOTH_M * SMOOTH_CUSPS), hue];
        }
        cusp_table[LOWER_WRAP] = [
            cusp_table[LAST_NOMINAL][0],
            cusp_table[LAST_NOMINAL][1],
            hue_table[LOWER_WRAP],
        ];
        cusp_table[UPPER_WRAP] = [
            cusp_table[FIRST_NOMINAL][0],
            cusp_table[FIRST_NOMINAL][1],
            hue_table[UPPER_WRAP],
        ];
        // `determine_hue_linearity_search_range`.
        let mut search_range = [0i32, 1i32];
        for (i, cusp) in cusp_table
            .iter()
            .enumerate()
            .take(UPPER_WRAP)
            .skip(FIRST_NOMINAL)
        {
            let pos = FIRST_NOMINAL as i32 + cusp[2] as i32;
            let delta = i as i32 - pos;
            search_range[0] = search_range[0].min(delta);
            search_range[1] = search_range[1].max(delta + 1);
        }
        // `make_upper_hull_gamma`.
        for i in FIRST_NOMINAL..UPPER_WRAP {
            let hue = hue_table[i];
            let jm = [cusp_table[i][0], cusp_table[i][1]];
            let fits = |gamma: f32| {
                gamma_fits(
                    jm,
                    hue,
                    1.0 / gamma,
                    peak,
                    limit_j_max,
                    mid_j,
                    focus_dist,
                    lower_hull_gamma_inv,
                    &p_out,
                )
            };
            let mut low = GAMMA_MINIMUM;
            let mut high = low + GAMMA_SEARCH_STEP;
            let mut outside = false;
            while !outside && high < GAMMA_MAXIMUM {
                if fits(high) {
                    outside = true;
                } else {
                    low = high;
                    high += GAMMA_SEARCH_STEP;
                }
            }
            while high - low > GAMMA_ACCURACY {
                let test = (high + low) / 2.0;
                if fits(test) {
                    high = test;
                } else {
                    low = test;
                }
            }
            cusp_table[i][2] = 1.0 / high;
        }
        cusp_table[LOWER_WRAP][2] = cusp_table[LAST_NOMINAL][2];
        cusp_table[UPPER_WRAP][2] = cusp_table[FIRST_NOMINAL][2];
        Ok(OutputTransform20 {
            p_in,
            p_out,
            tone,
            shared,
            chroma,
            gamut: GamutCompress {
                mid_j,
                focus_dist,
                lower_hull_gamma_inv,
                search_range,
                hue_table,
                cusp_table,
            },
        })
    }

    /// `Renderer_ACES_OutputTransform20::fwd`: AP0 scene linear → limiting-primaries display
    /// linear.
    pub fn apply(&self, rgb: F3) -> F3 {
        let aab = self.p_in.rgb_to_aab(rgb);
        let jmh = self.p_in.aab_to_jmh(aab);
        let sr = Resolved {
            limit_j_max: self.shared.limit_j_max,
            model_gamma_inv: self.shared.model_gamma_inv,
            reach_max_m: reach_m_from_table(jmh[2], &self.shared.reach_m_table),
        };
        let h_rad = to_radians(jmh[2]);
        let (cos_hr, sin_hr) = (libm::cosf(h_rad), libm::sinf(h_rad));
        let m_norm = chroma_compress_norm(cos_hr, sin_hr, self.chroma.scale);

        // `tonescale_A_to_J_fwd`.
        let y_out = self.tone.apply(self.p_in.a_to_y(aab[0]));
        let j_ts = libm::copysignf(self.p_in.y_to_j_abs(y_out), aab[0]);

        // `chroma_compress_fwd`.
        let [j, m, h] = jmh;
        let mut m_cp = m;
        if m != 0.0 {
            let nj = j_ts / sr.limit_j_max;
            let snj = (1.0 - nj).max(0.0);
            let limit = libm::powf(nj, sr.model_gamma_inv) * sr.reach_max_m / m_norm;
            m_cp = m * libm::powf(j_ts / j, sr.model_gamma_inv);
            m_cp /= m_norm;
            m_cp = limit
                - toe_fwd(
                    limit - m_cp,
                    limit - 0.001,
                    snj * self.chroma.sat,
                    libm::sqrtf(nj * nj + self.chroma.sat_thr),
                );
            m_cp = toe_fwd(m_cp, limit, nj * self.chroma.compr, snj);
            m_cp *= m_norm;
        }
        let compressed = self.gamut.forward([j_ts, m_cp, h], &sr);
        let aab_out = self.p_out.jmh_to_aab_cs(compressed, cos_hr, sin_hr);
        self.p_out.aab_to_rgb(aab_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec709_100() -> OutputTransform20 {
        OutputTransform20::new(&[100.0, 0.64, 0.33, 0.3, 0.6, 0.15, 0.06, 0.3127, 0.329]).unwrap()
    }

    #[test]
    fn primaries_matrices_round_trip() {
        let m = rgb_to_xyz_f33(&AP0);
        let inv = xyz_to_rgb_f33(&AP0);
        let id = mul_m_m(&inv, &m);
        for (i, v) in id.iter().enumerate() {
            let want = if i % 4 == 0 { 1.0 } else { 0.0 };
            assert!((v - want).abs() < 1e-6, "{id:?}");
        }
        // The AP0 white (Y = 1) maps to the ACES white point.
        let w = mul_m_v([1.0; 3], &m);
        assert!((w[1] - 1.0).abs() < 1e-6);
        assert!((w[0] / (w[0] + w[1] + w[2]) - 0.32168).abs() < 1e-5);
    }

    #[test]
    fn jmh_round_trips_and_greys_are_achromatic() {
        let p = JMhParams::new(&AP0);
        for rgb in [[0.18f32, 0.18, 0.18], [0.5, 0.2, 0.1], [2.0, 4.0, 0.5]] {
            let jmh = p.rgb_to_jmh(rgb);
            let back = p.jmh_to_rgb(jmh);
            for c in 0..3 {
                assert!(
                    (back[c] - rgb[c]).abs() < 1e-4 * rgb[c].max(1.0),
                    "{rgb:?} {back:?}"
                );
            }
        }
        assert!(p.rgb_to_jmh([0.18; 3])[1] < 1e-3);
    }

    #[test]
    fn output_transform_is_monotonic_on_greys_and_maps_mid_grey_near_10_nits() {
        let t = rec709_100();
        let mut last = -1.0f32;
        for i in 0..60 {
            let x = libm::exp2f(-12.0 + i as f32 * 0.4);
            let o = t.apply([x; 3]);
            assert!(o.iter().all(|v| v.is_finite()));
            assert!(o[1] >= last - 1e-6, "{x} -> {o:?}");
            last = o[1];
        }
        // 18 % grey → ~10 nits (0.1) of 100 nits peak; highlights approach the peak.
        let g = t.apply([0.18; 3])[1];
        assert!((g - 0.1).abs() < 0.01, "{g}");
        let hi = t.apply([2000.0; 3]);
        // Slightly above 1 before the pipeline's Range clamp, as in OCIO.
        assert!(hi[1] > 0.95 && hi[1] <= 1.02, "{hi:?}");
        assert_eq!(t.apply([0.0; 3]), [0.0; 3]);
        // The tables are complete and increasing in hue.
        for i in 1..TOTAL_SIZE {
            assert!(t.gamut.hue_table[i] > t.gamut.hue_table[i - 1]);
        }
    }
}
