// SPDX-License-Identifier: GPL-3.0-or-later
//! The default falloff curve of a WARP modifier (`BKE_curvemapping_add(1, 0, 0, 1, 1)`: two
//! auto-handle points (0, 0) and (1, 1), clipped to [0, 1], extrapolated) evaluated as Blender
//! does: through its 257-entry table (`curvemap_make_table`) and `BKE_curvemapping_evaluateF`.
//!
//! Ported from blender/source/blender/blenkernel/intern/colortools.cc
//! (`BKE_curvemapping_set_defaults`, `calchandle_curvemap`, `curvemap_make_table`,
//! `curvemap_calc_extend`, `BKE_curvemap_evaluateF`, `BKE_curvemapping_evaluateF`) and
//! blender/source/blender/blenkernel/intern/curve.cc (`BKE_curve_correct_bezpart`,
//! `BKE_curve_forward_diff_bezier`), Blender 5.2.4, GPL-2.0-or-later.

use std::sync::OnceLock;

/// `CM_RESOL`
const CM_RESOL: usize = 32;
/// `CM_TABLE`
const CM_TABLE: usize = 256;

type V2 = [f32; 2];

/// A Bézier key: left handle, point, right handle.
#[derive(Copy, Clone, Default)]
struct Bezt {
    vec: [V2; 3],
}

fn len_v2(v: V2) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

/// `calchandle_curvemap` for `HD_AUTO` handles.
fn calchandle(bezt: &mut Bezt, prev: Option<V2>, next: Option<V2>) {
    let p2 = bezt.vec[1];
    let (p1, p3) = match (prev, next) {
        (None, Some(n)) => ([2.0 * p2[0] - n[0], 2.0 * p2[1] - n[1]], n),
        (Some(p), None) => (p, [2.0 * p2[0] - p[0], 2.0 * p2[1] - p[1]]),
        (Some(p), Some(n)) => (p, n),
        (None, None) => return,
    };
    let dvec_a = [p2[0] - p1[0], p2[1] - p1[1]];
    let dvec_b = [p3[0] - p2[0], p3[1] - p2[1]];
    let mut len_a = len_v2(dvec_a);
    let mut len_b = len_v2(dvec_b);
    if len_a == 0.0 {
        len_a = 1.0;
    }
    if len_b == 0.0 {
        len_b = 1.0;
    }
    let tvec = [
        dvec_b[0] / len_b + dvec_a[0] / len_a,
        dvec_b[1] / len_b + dvec_a[1] / len_a,
    ];
    let len = len_v2(tvec) * 2.5614;
    if len != 0.0 {
        len_a /= len;
        bezt.vec[0] = [p2[0] + tvec[0] * -len_a, p2[1] + tvec[1] * -len_a];
        len_b /= len;
        bezt.vec[2] = [p2[0] + tvec[0] * len_b, p2[1] + tvec[1] * len_b];
    }
}

/// `BKE_curve_correct_bezpart`
fn correct_bezpart(v1: V2, v2: &mut V2, v3: &mut V2, v4: V2) {
    let h1 = [v1[0] - v2[0], v1[1] - v2[1]];
    let h2 = [v4[0] - v3[0], v4[1] - v3[1]];
    let len = v4[0] - v1[0];
    let len1 = h1[0].abs();
    let len2 = h2[0].abs();
    if len1 + len2 == 0.0 {
        return;
    }
    if len1 + len2 > len {
        let fac = len / (len1 + len2);
        *v2 = [v1[0] - fac * h1[0], v1[1] - fac * h1[1]];
        *v3 = [v4[0] - fac * h2[0], v4[1] - fac * h2[1]];
    }
}

/// `BKE_curve_forward_diff_bezier` into `out[..][k]` (`it + 1` values).
fn forward_diff_bezier(q: [f32; 4], out: &mut [V2], k: usize, it: usize) {
    let [mut q0, mut q1, mut q2, mut q3] = q;
    let mut f = it as f32;
    let rt0 = q0;
    let rt1 = 3.0 * (q1 - q0) / f;
    f *= f;
    let rt2 = 3.0 * (q0 - 2.0 * q1 + q2) / f;
    f *= it as f32;
    let rt3 = (q3 - q0 + 3.0 * (q1 - q2)) / f;
    q0 = rt0;
    q1 = rt1 + rt2 + rt3;
    q2 = 2.0 * rt2 + 6.0 * rt3;
    q3 = 6.0 * rt3;
    for p in out.iter_mut().take(it + 1) {
        p[k] = q0;
        q0 += q1;
        q1 += q2;
        q2 += q3;
    }
}

/// The evaluated table and extrapolation of the default curve.
struct Table {
    table: Vec<V2>,
    mintable: f32,
    range: f32,
    ext_in: V2,
    ext_out: V2,
}

/// `curvemap_calc_extend` with `CUMA_EXTEND_EXTRAPOLATE`.
fn calc_extend(t: &Table, x: f32, first: V2, last: V2) -> f32 {
    if x <= first[0] {
        if t.ext_in[0] == 0.0 {
            return first[1] + t.ext_in[1] * 10000.0;
        }
        return first[1] + t.ext_in[1] * (x - first[0]) / t.ext_in[0];
    }
    if x >= last[0] {
        if t.ext_out[0] == 0.0 {
            return last[1] - t.ext_out[1] * 10000.0;
        }
        return last[1] + t.ext_out[1] * (x - last[0]) / t.ext_out[0];
    }
    0.0
}

/// `curvemap_make_table` of the default two-point curve.
fn make_table() -> Table {
    let mut bezt = [Bezt::default(); 2];
    bezt[0].vec[1] = [0.0, 0.0];
    bezt[1].vec[1] = [1.0, 1.0];
    let (mintable, maxtable) = (0.0f32, 1.0f32);
    let p1 = bezt[1].vec[1];
    calchandle(&mut bezt[0], None, Some(p1));
    let p0 = bezt[0].vec[1];
    calchandle(&mut bezt[1], Some(p0), None);
    // The Bézier segment, `CM_RESOL` points.
    let mut all = vec![[0.0f32; 2]; CM_RESOL];
    {
        let mut start = bezt[0].vec;
        let mut end = bezt[1].vec;
        let (s1, e1) = (start[1], end[1]);
        correct_bezpart(s1, &mut start[2], &mut end[0], e1);
        for k in 0..2 {
            forward_diff_bezier(
                [start[1][k], start[2][k], end[0][k], end[1][k]],
                &mut all,
                k,
                CM_RESOL - 1,
            );
        }
    }
    let unit = |v: V2| {
        let r = (v[0] * v[0] + v[1] * v[1]).sqrt();
        [v[0] / r, v[1] / r]
    };
    let ext_in = unit([
        bezt[0].vec[0][0] - bezt[0].vec[1][0],
        bezt[0].vec[0][1] - bezt[0].vec[1][1],
    ]);
    let ext_out = unit([
        bezt[1].vec[1][0] - bezt[1].vec[2][0],
        bezt[1].vec[1][1] - bezt[1].vec[2][1],
    ]);
    let range = (1.0f32 / 256.0) * (maxtable - mintable);
    let mut t = Table {
        table: Vec::with_capacity(CM_TABLE + 1),
        mintable,
        range: 1.0 / range,
        ext_in,
        ext_out,
    };
    let (first, last) = (all[0], all[CM_RESOL - 1]);
    let mut pi = 0usize;
    for a in 0..=CM_TABLE {
        let cur_x = mintable + range * a as f32;
        // The first point with an x larger than `cur_x`.
        while cur_x >= all[pi][0] && pi != CM_RESOL - 1 {
            pi += 1;
        }
        let point = all[pi];
        let y = if pi == 0 || (pi == CM_RESOL - 1 && cur_x >= point[0]) {
            if (cur_x - point[0]).abs() <= 1e-6 {
                point[1]
            } else {
                calc_extend(&t, cur_x, first, last)
            }
        } else {
            let prev = all[pi - 1];
            let mut fac1 = point[0] - prev[0];
            let fac2 = point[0] - cur_x;
            fac1 = if fac1 > f32::EPSILON {
                fac2 / fac1
            } else {
                0.0
            };
            fac1 * prev[1] + (1.0 - fac1) * point[1]
        };
        t.table.push([cur_x, y]);
    }
    t
}

/// `BKE_curvemapping_evaluateF(curfalloff, 0, value)` of the default curve.
pub(crate) fn evaluate_default(value: f32) -> f32 {
    static TABLE: OnceLock<Table> = OnceLock::new();
    let t = TABLE.get_or_init(make_table);
    let mut fi = (value - t.mintable) * t.range;
    let i = fi as i32;
    let val = if !(0.0..=CM_TABLE as f32).contains(&fi) {
        calc_extend(t, value, t.table[0], t.table[CM_TABLE])
    } else if i < 0 {
        t.table[0][1]
    } else if i as usize >= CM_TABLE {
        t.table[CM_TABLE][1]
    } else {
        let i = i as usize;
        fi -= i as f32;
        (1.0 - fi) * t.table[i][1] + fi * t.table[i + 1][1]
    };
    // `CUMA_DO_CLIP` to [0, 1].
    val.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_curve_is_close_to_the_identity() {
        for k in 0..=100 {
            let x = k as f32 / 100.0;
            assert!((evaluate_default(x) - x).abs() < 1e-5, "{x}");
        }
        assert_eq!(evaluate_default(0.0), 0.0);
        assert_eq!(evaluate_default(1.0), 1.0);
    }
}
