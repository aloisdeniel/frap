// SPDX-License-Identifier: GPL-3.0-or-later
//! Handle recalculation: `BKE_fcurve_handles_recalc_ex` → `calchandleNurb_intern` (F-curve
//! mode) per key, then `BKE_nurb_handle_smooth_fcurve` (continuous-acceleration smoothing, a
//! clamped tridiagonal solve) when the curve's `auto_smoothing` is `CONT_ACCEL`.
//!
//! Ported from blender/source/blender/blenkernel/intern/fcurve.cc, curve.cc and
//! blenlib/intern/math_solvers.cc (Blender 5.2.4, GPL-2.0-or-later). frap's F-curves have no
//! F-modifiers, so the "Cycles modifier" smoothing through the cycle never applies.

use crate::fcurve::{Extrapolation, FCurve, HandleType, Keyframe};

/// `HD_AUTOTYPE_*`: `false` = `NORMAL`, `true` = `LOCKED_FINAL`.
type Locked = bool;

/// `increment_ulp`
fn increment_ulp(v: f32) -> f32 {
    if !v.is_finite() {
        return v;
    }
    if v > 0.0 {
        f32::from_bits(v.to_bits() + 1)
    } else if v < -0.0 {
        f32::from_bits(v.to_bits() - 1)
    } else {
        f32::from_bits(0x0000_0001)
    }
}

/// `decrement_ulp`
fn decrement_ulp(v: f32) -> f32 {
    if !v.is_finite() {
        return v;
    }
    if v > 0.0 {
        f32::from_bits(v.to_bits() - 1)
    } else if v < -0.0 {
        f32::from_bits(v.to_bits() + 1)
    } else {
        f32::from_bits(0x8000_0001)
    }
}

/// `len_v3v3` of two F-curve points (z = 0).
fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1]];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

/// `madd_v3_v3v3fl`: `a + b * f`.
fn madd(a: [f32; 2], b: [f32; 2], f: f32) -> [f32; 2] {
    [a[0] + b[0] * f, a[1] + b[1] * f]
}

/// `calchandleNurb_intern(bezt, prev, next, BEZT_FLAG_SELECT, is_fcurve = true,
/// skip_align = false, smoothing)`; `prev` / `next` are the neighbours' control points.
fn calchandle(
    bezt: &mut Keyframe,
    prev: Option<[f32; 2]>,
    next: Option<[f32; 2]>,
    smoothing: bool,
) -> Locked {
    use HandleType::{Aligned, AutoClamped, Free, Vector};
    let eps = 1e-5_f32;
    let mut locked = false;
    if bezt.handle_left_type == Free && bezt.handle_right_type == Free {
        return locked;
    }
    let p2 = bezt.co;
    let (p1, p3) = match (prev, next) {
        (Some(p1), Some(p3)) => (p1, p3),
        (None, Some(p3)) => ([2.0 * p2[0] - p3[0], 2.0 * p2[1] - p3[1]], p3),
        (Some(p1), None) => (p1, [2.0 * p2[0] - p1[0], 2.0 * p2[1] - p1[1]]),
        (None, None) => return locked,
    };
    let dvec_a = [p2[0] - p1[0], p2[1] - p1[1]];
    let dvec_b = [p3[0] - p2[0], p3[1] - p2[1]];
    let mut len_a = dvec_a[0];
    let mut len_b = dvec_b[0];
    if len_a == 0.0 {
        len_a = 1.0;
    }
    if len_b == 0.0 {
        len_b = 1.0;
    }
    let (h1, h2) = (bezt.handle_left_type, bezt.handle_right_type);
    if h1.is_auto() || h2.is_auto() {
        let tvec = [
            dvec_b[0] / len_b + dvec_a[0] / len_a,
            dvec_b[1] / len_b + dvec_a[1] / len_a,
        ];
        let mut len = if smoothing { 6.0 / 2.5614 } else { tvec[0] };
        len *= 2.5614;
        if len != 0.0 {
            let mut leftviolate = false;
            let mut rightviolate = false;
            if !smoothing {
                len_a = len_a.min(5.0 * len_b);
                len_b = len_b.min(5.0 * len_a);
            }
            if h1.is_auto() {
                len_a /= len;
                bezt.handle_left = madd(p2, tvec, -len_a);
                if let (AutoClamped, Some(next), Some(prev)) = (h1, next, prev) {
                    let ydiff1 = prev[1] - p2[1];
                    let ydiff2 = next[1] - p2[1];
                    if (ydiff1 <= 0.0 && ydiff2 <= 0.0) || (ydiff1 >= 0.0 && ydiff2 >= 0.0) {
                        bezt.handle_left[1] = p2[1];
                        locked = true;
                    } else if ydiff1 <= 0.0 {
                        if prev[1] > bezt.handle_left[1] {
                            bezt.handle_left[1] = prev[1];
                            leftviolate = true;
                        }
                    } else if prev[1] < bezt.handle_left[1] {
                        bezt.handle_left[1] = prev[1];
                        leftviolate = true;
                    }
                }
            }
            if h2.is_auto() {
                len_b /= len;
                bezt.handle_right = madd(p2, tvec, len_b);
                if let (AutoClamped, Some(next), Some(prev)) = (h2, next, prev) {
                    let ydiff1 = prev[1] - p2[1];
                    let ydiff2 = next[1] - p2[1];
                    if (ydiff1 <= 0.0 && ydiff2 <= 0.0) || (ydiff1 >= 0.0 && ydiff2 >= 0.0) {
                        bezt.handle_right[1] = p2[1];
                        locked = true;
                    } else if ydiff1 <= 0.0 {
                        if next[1] < bezt.handle_right[1] {
                            bezt.handle_right[1] = next[1];
                            rightviolate = true;
                        }
                    } else if next[1] > bezt.handle_right[1] {
                        bezt.handle_right[1] = next[1];
                        rightviolate = true;
                    }
                }
            }
            if leftviolate || rightviolate {
                let h1_x = bezt.handle_left[0] - p2[0];
                let h2_x = p2[0] - bezt.handle_right[0];
                if leftviolate {
                    bezt.handle_right[1] = p2[1] + ((p2[1] - bezt.handle_left[1]) / h1_x) * h2_x;
                } else {
                    bezt.handle_left[1] = p2[1] + ((p2[1] - bezt.handle_right[1]) / h2_x) * h1_x;
                }
            }
        }
    }
    if h1 == Vector {
        bezt.handle_left = madd(p2, dvec_a, -1.0 / 3.0);
    }
    if h2 == Vector {
        bezt.handle_right = madd(p2, dvec_b, 1.0 / 3.0);
    }
    // When one handle is free, or none is aligned, aligning makes no sense.
    if h1 == Free || h2 == Free || (h1 != Aligned && h2 != Aligned) {
        return locked;
    }
    let mut len_a = dist(p2, bezt.handle_left);
    let mut len_b = dist(p2, bezt.handle_right);
    if len_a == 0.0 {
        len_a = 1.0;
    }
    if len_b == 0.0 {
        len_b = 1.0;
    }
    let len_ratio = len_a / len_b;
    let align_right = |b: &mut Keyframe| {
        if len_a > eps {
            let len = 1.0 / len_ratio;
            b.handle_right = [
                p2[0] + len * (p2[0] - b.handle_left[0]),
                p2[1] + len * (p2[1] - b.handle_left[1]),
            ];
        }
    };
    let align_left = |b: &mut Keyframe| {
        if len_b > eps {
            let len = len_ratio;
            b.handle_left = [
                p2[0] + len * (p2[0] - b.handle_right[0]),
                p2[1] + len * (p2[1] - b.handle_right[1]),
            ];
        }
    };
    // Order of calculation: the selected left handle wins.
    if bezt.select_left_handle {
        if h2 == Aligned {
            align_right(bezt);
        }
        if h1 == Aligned {
            align_left(bezt);
        }
    } else {
        if h1 == Aligned {
            align_left(bezt);
        }
        if h2 == Aligned {
            align_right(bezt);
        }
    }
    locked
}

/// `BKE_fcurve_handles_recalc` (`BKE_fcurve_handles_recalc_ex(fcu, BEZT_FLAG_SELECT)`).
pub fn recalc(fc: &mut FCurve) {
    let n = fc.keyframe_points.len();
    if n < 2 {
        return;
    }
    let smoothing = fc.smooths();
    let constant = fc.extrapolation == Extrapolation::Constant;
    let mut locked: Vec<Locked> = vec![false; n];
    let keys = &mut fc.keyframe_points;
    for i in 0..n {
        let prev = (i > 0).then(|| keys[i - 1].co);
        let next = (i + 1 < n).then(|| keys[i + 1].co);
        let bezt = &mut keys[i];
        // Keep handle times on their side of the key (with a margin, #141029).
        let threshold = 0.001_f32;
        bezt.handle_left[0] = bezt.handle_left[0].min(decrement_ulp(bezt.co[0] - threshold));
        bezt.handle_right[0] = bezt.handle_right[0].max(increment_ulp(bezt.co[0] + threshold));
        locked[i] = calchandle(bezt, prev, next, smoothing);
        // Automatic ease in and out on the first and last key.
        if bezt.is_auto_h() && (i == 0 || i == n - 1) && constant {
            bezt.handle_left[1] = bezt.co[1];
            bezt.handle_right[1] = bezt.co[1];
            locked[i] = true;
        }
        // Avoid total smoothing failure on duplicate keyframes.
        if let Some(p) = prev
            && p[0] >= bezt.co[0]
        {
            locked[i - 1] = true;
            locked[i] = true;
        }
    }
    if smoothing {
        smooth_fcurve(keys, &locked);
    }
}

// ------------------------------------------------------------------------------- smoothing

/// `is_free_auto_point`
fn is_free_auto_point(b: &Keyframe, locked: Locked) -> bool {
    b.is_auto_h() && !locked
}

/// `BKE_nurb_handle_smooth_fcurve(bezt, total, cyclic = false)`: smooths every run of free
/// auto handles.
fn smooth_fcurve(keys: &mut [Keyframe], locked: &[Locked]) {
    let total = keys.len();
    let mut start = 0;
    let mut count = 1;
    for j in 1..total {
        // A non-auto handle closes the run (always reached for the last handle).
        if !is_free_auto_point(&keys[j], locked[j]) {
            smooth_run(keys, locked, start, count + 1);
            start = j;
            count = 1;
        } else {
            count += 1;
        }
    }
    if count > 1 {
        smooth_run(keys, locked, start, count);
    }
}

/// `bezier_check_solve_end_handle`
fn check_solve_end_handle(htype: HandleType, end: bool, locked: Locked) -> bool {
    htype == HandleType::Vector || (end && htype.is_auto() && !locked)
}

/// `bezier_calc_handle_adj`: scales `hsize` when the handles intersect in x; returns the
/// correction of the equation for a handle whose x size is not a third of the interval.
fn calc_handle_adj(hsize: &mut [f32; 2], dx: f32) -> f32 {
    let fac = dx / (hsize[0] + dx / 3.0);
    if fac < 1.0 {
        hsize[0] *= fac;
        hsize[1] *= fac;
    }
    1.0 - 3.0 * hsize[0] / dx
}

/// The tridiagonal system `a[i] h[i-1] + b[i] h[i] + c[i] h[i+1] = d[i]` with per-unknown
/// limits.
struct System {
    a: Vec<f32>,
    b: Vec<f32>,
    c: Vec<f32>,
    d: Vec<f32>,
}

impl System {
    /// `bezier_lock_unknown`
    fn lock(&mut self, i: usize, value: f32) {
        self.a[i] = 0.0;
        self.c[i] = 0.0;
        self.b[i] = 1.0;
        self.d[i] = value;
    }
    /// `bezier_eq_continuous`
    fn continuous(&mut self, dy: &[f32], l: &[f32], i: usize) {
        self.a[i] = l[i] * l[i];
        self.b[i] = 2.0 * (l[i] + 1.0);
        self.c[i] = 1.0 / l[i + 1];
        self.d[i] = dy[i] * l[i] * l[i] + dy[i + 1];
    }
    /// `bezier_eq_noaccel_right`
    fn noaccel_right(&mut self, dy: &[f32], l: &[f32], i: usize) {
        self.a[i] = 0.0;
        self.b[i] = 2.0;
        self.c[i] = 1.0 / l[i + 1];
        self.d[i] = dy[i + 1];
    }
    /// `bezier_eq_noaccel_left`
    fn noaccel_left(&mut self, dy: &[f32], l: &[f32], i: usize) {
        self.a[i] = l[i] * l[i];
        self.b[i] = 2.0 * l[i];
        self.c[i] = 0.0;
        self.d[i] = dy[i] * l[i] * l[i];
    }
}

/// `bezier_clamp`: auto clamp keeps a point from going the wrong way and adjacent handles
/// from overshooting.
fn clamp(
    hmax: &mut [f32],
    hmin: &mut [f32],
    i: usize,
    dy: f32,
    no_reverse: bool,
    no_overshoot: bool,
) {
    if dy > 0.0 {
        if no_overshoot {
            hmax[i] = hmax[i].min(dy);
        }
        if no_reverse {
            hmin[i] = 0.0;
        }
    } else if dy < 0.0 {
        if no_reverse {
            hmax[i] = 0.0;
        }
        if no_overshoot {
            hmin[i] = hmin[i].max(dy);
        }
    } else if no_reverse || no_overshoot {
        hmax[i] = 0.0;
        hmin[i] = 0.0;
    }
}

/// `bezier_output_handle_inner`
fn output_handle_inner(b: &mut Keyframe, right: bool, newval: [f32; 2], endpoint: bool) {
    let (hr, hm) = if right {
        (b.handle_right_type, b.handle_left_type)
    } else {
        (b.handle_left_type, b.handle_right_type)
    };
    // Only assign Auto/Vector handles.
    if !(hr.is_auto() || hr == HandleType::Vector) {
        return;
    }
    let co = b.co;
    let (this, other) = if right {
        (&mut b.handle_right, &mut b.handle_left)
    } else {
        (&mut b.handle_left, &mut b.handle_right)
    };
    *this = newval;
    if hm == HandleType::Aligned {
        // Fix up the aligned handle.
        let hlen = dist(co, *other);
        let h2len = dist(co, *this);
        let tmp = [co[0] - this[0], co[1] - this[1]];
        *other = madd(co, tmp, hlen / h2len);
    } else if endpoint && (hm.is_auto() || hm == HandleType::Vector) {
        // At the curve's end points, mirror the handle to the other side.
        let tmp = [co[0] - this[0], co[1] - this[1]];
        *other = [co[0] + tmp[0], co[1] + tmp[1]];
    }
}

/// `bezier_output_handle`
fn output_handle(b: &mut Keyframe, right: bool, dy: f32, endpoint: bool) {
    let mut tmp = if right { b.handle_right } else { b.handle_left };
    tmp[1] = b.co[1] + dy;
    output_handle_inner(b, right, tmp, endpoint);
}

/// `bezier_handle_calc_smooth_fcurve(bezt, total, start, count, cycle = false)`: handles
/// with no acceleration discontinuity through the run `start .. start + count`.
fn smooth_run(keys: &mut [Keyframe], locked: &[Locked], start: usize, count: usize) {
    let total = keys.len();
    if count < 2 {
        return;
    }
    let first = start;
    let last = start + count - 1;
    let solve_first =
        check_solve_end_handle(keys[first].handle_right_type, start == 0, locked[first]);
    let solve_last = check_solve_end_handle(
        keys[last].handle_left_type,
        start + count == total,
        locked[last],
    );
    if count == 2 && solve_first == solve_last {
        return;
    }
    // Point locations.
    let mut dx = vec![f32::NAN; count];
    let mut dy = vec![f32::NAN; count];
    for i in 1..count {
        let j = start + i;
        dx[i] = keys[j].co[0] - keys[j - 1].co[0];
        dy[i] = keys[j].co[1] - keys[j - 1].co[1];
    }
    // Ratio of x intervals.
    let mut l = vec![0.0_f32; count];
    l[0] = 1.0;
    l[count - 1] = 1.0;
    for i in 1..count - 1 {
        l[i] = dx[i + 1] / dx[i];
    }
    // Handle clamp ranges.
    let clamped = |k: &Keyframe| {
        k.handle_left_type == HandleType::AutoClamped
            || k.handle_right_type == HandleType::AutoClamped
    };
    let mut hmax = vec![f32::MAX; count];
    let mut hmin = vec![-f32::MAX; count];
    let mut clamped_cur = clamped(&keys[first]);
    for i in 1..count {
        let j = start + i;
        let clamped_prev = clamped_cur;
        clamped_cur = clamped(&keys[j]);
        clamp(
            &mut hmax,
            &mut hmin,
            i - 1,
            dy[i],
            clamped_prev,
            clamped_prev,
        );
        clamp(
            &mut hmax,
            &mut hmin,
            i,
            dy[i] * l[i],
            clamped_cur,
            clamped_cur,
        );
    }
    let mut sys = System {
        a: vec![0.0; count],
        b: vec![0.0; count],
        c: vec![0.0; count],
        d: vec![0.0; count],
    };
    // Boundary conditions: fixed handles or zero curvature.
    let mut first_handle_adj = 0.0;
    let mut last_handle_adj = 0.0;
    if !solve_first {
        let b = &keys[first];
        let mut tmp = [b.handle_right[0] - b.co[0], b.handle_right[1] - b.co[1]];
        first_handle_adj = calc_handle_adj(&mut tmp, dx[1]);
        sys.lock(0, tmp[1]);
    } else {
        sys.noaccel_right(&dy, &l, 0);
    }
    if !solve_last {
        let b = &keys[last];
        let mut tmp = [b.co[0] - b.handle_left[0], b.co[1] - b.handle_left[1]];
        last_handle_adj = calc_handle_adj(&mut tmp, dx[count - 1]);
        sys.lock(count - 1, tmp[1]);
    } else {
        sys.noaccel_left(&dy, &l, count - 1);
    }
    // Main tridiagonal system.
    for i in 1..count - 1 {
        sys.continuous(&dy, &l, i);
    }
    // Correction for user-defined handles with non-standard x positions.
    if count > 2 || solve_last {
        sys.b[1] += l[1] * first_handle_adj;
    }
    if count > 2 || solve_first {
        sys.b[count - 2] += last_handle_adj;
    }
    let mut h = vec![0.0_f32; count];
    if solve_with_limits(&mut sys, &mut h, &hmin, &hmax) {
        for i in 1..count - 1 {
            let j = start + i;
            let end = j == total - 1;
            output_handle(&mut keys[j], false, -h[i] / l[i], end);
            output_handle(&mut keys[j], true, h[i], end);
        }
        if solve_first {
            output_handle(&mut keys[first], true, h[0], start == 0);
        }
        if solve_last {
            output_handle(
                &mut keys[last],
                false,
                -h[count - 1] / l[count - 1],
                start + count == total,
            );
        }
    }
}

/// `bezier_relax_direction`: which way to move `h[i]` to better satisfy its equation.
fn relax_direction(a: &[f32], b: &[f32], c: &[f32], d: &[f32], h: &[f32], i: usize) -> f32 {
    let n = h.len();
    let state = a[i] * h[(i + n - 1) % n] + b[i] * h[i] + c[i] * h[(i + 1) % n] - d[i];
    -state * b[i]
}

/// `tridiagonal_solve_with_limits`
fn solve_with_limits(sys: &mut System, h: &mut [f32], hmin: &[f32], hmax: &[f32]) -> bool {
    let n = h.len();
    let (a0, b0, c0, d0) = (sys.a.clone(), sys.b.clone(), sys.c.clone(), sys.d.clone());
    let mut is_locked = vec![false; n];
    let mut num_unlocks = vec![0_u8; n];
    loop {
        if !tridiagonal_solve_cyclic(&sys.a, &sys.b, &sys.c, &sys.d, h) {
            return false;
        }
        // Lock the handles that overshoot their limits.
        let mut all = false;
        let mut locked = false;
        let mut overshoot;
        loop {
            overshoot = false;
            for i in 0..n {
                if h[i] >= hmin[i] && h[i] <= hmax[i] {
                    continue;
                }
                overshoot = true;
                let target = if h[i] > hmax[i] { hmax[i] } else { hmin[i] };
                // Only lock handles that go in the right direction, if there are such.
                if target != 0.0 || all {
                    is_locked[i] = true;
                    sys.lock(i, target);
                    locked = true;
                }
            }
            all = true;
            if !(overshoot && !locked) {
                break;
            }
        }
        // Without new locks, see whether some locked handles may be released.
        let mut unlocked = false;
        if !locked {
            for i in 0..n {
                // At most twice, to avoid infinite loops.
                if !is_locked[i] || num_unlocks[i] >= 2 {
                    continue;
                }
                let relax = relax_direction(&a0, &b0, &c0, &d0, h, i);
                if (relax > 0.0 && h[i] < hmax[i]) || (relax < 0.0 && h[i] > hmin[i]) {
                    sys.a[i] = a0[i];
                    sys.b[i] = b0[i];
                    sys.c[i] = c0[i];
                    sys.d[i] = d0[i];
                    is_locked[i] = false;
                    num_unlocks[i] += 1;
                    unlocked = true;
                }
            }
        }
        if !(overshoot || unlocked) {
            return true;
        }
    }
}

/// `BLI_tridiagonal_solve` (Thomas algorithm, `double` intermediates).
fn tridiagonal_solve(a: &[f32], b: &[f32], c: &[f32], d: &[f32], x: &mut [f32]) -> bool {
    let n = x.len();
    if n < 1 {
        return false;
    }
    let mut c1 = vec![0.0_f64; n];
    let mut d1 = vec![0.0_f64; n];
    let mut c_prev = f64::from(c[0]) / f64::from(b[0]);
    let mut d_prev = f64::from(d[0]) / f64::from(b[0]);
    c1[0] = c_prev;
    d1[0] = d_prev;
    for i in 1..n {
        let denum = f64::from(b[i]) - f64::from(a[i]) * c_prev;
        c_prev = f64::from(c[i]) / denum;
        d_prev = (f64::from(d[i]) - f64::from(a[i]) * d_prev) / denum;
        c1[i] = c_prev;
        d1[i] = d_prev;
    }
    let mut x_prev = d_prev;
    x[n - 1] = x_prev as f32;
    for i in (0..n - 1).rev() {
        x_prev = d1[i] - c1[i] * x_prev;
        x[i] = x_prev as f32;
    }
    x_prev.is_finite()
}

/// `BLI_tridiagonal_solve_cyclic`
fn tridiagonal_solve_cyclic(a: &[f32], b: &[f32], c: &[f32], d: &[f32], x: &mut [f32]) -> bool {
    let n = x.len();
    if n < 1 {
        return false;
    }
    if n == 1 {
        x[0] = d[0] / (a[0] + b[0] + c[0]);
        return x[0].is_finite();
    }
    if n == 2 {
        let a2 = [0.0, a[1] + c[1]];
        let c2 = [a[0] + c[0], 0.0];
        return tridiagonal_solve(&a2, b, &c2, d, x);
    }
    let a0 = a[0];
    let cn = c[n - 1];
    if a0 == 0.0 && cn == 0.0 {
        return tridiagonal_solve(a, b, c, d, x);
    }
    let mut b2 = b.to_vec();
    b2[0] -= a0;
    b2[n - 1] -= cn;
    let mut tmp = vec![0.0_f32; n];
    tmp[0] = a0;
    tmp[n - 1] = cn;
    let rhs = tmp.clone();
    let ok = tridiagonal_solve(a, &b2, c, &rhs, &mut tmp) && tridiagonal_solve(a, &b2, c, d, x);
    if ok {
        let coeff = (x[0] + x[n - 1]) / (1.0 + tmp[0] + tmp[n - 1]);
        for i in 0..n {
            x[i] -= coeff * tmp[i];
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(keys: &[(f32, f32)], smoothing: &str) -> FCurve {
        let mut fc = FCurve::new("location", 0);
        fc.auto_smoothing = smoothing.into();
        fc.keyframe_points = keys
            .iter()
            .map(|(f, v)| {
                let mut k = Keyframe::new(*f, *v);
                k.handle_left = [*f - 1.0, *v];
                k.handle_right = [*f + 1.0, *v];
                k
            })
            .collect();
        fc
    }

    /// Two keys with constant extrapolation and the default smoothing: flat handles at a
    /// third of the interval (verified on 5.2.4: keys (1, 0) and (10, 5) → handles (-2, 0)
    /// (4, 0) and (7, 5) (13, 5)).
    #[test]
    fn two_keys_flat_ends() {
        let mut fc = curve(&[(1.0, 0.0), (10.0, 5.0)], "CONT_ACCEL");
        recalc(&mut fc);
        let k = &fc.keyframe_points;
        assert_eq!(k[0].handle_left, [-2.0, 0.0]);
        assert_eq!(k[0].handle_right, [4.0, 0.0]);
        assert_eq!(k[1].handle_left, [7.0, 5.0]);
        assert_eq!(k[1].handle_right, [13.0, 5.0]);
    }

    #[test]
    fn vector_handles_point_at_neighbours() {
        let mut fc = curve(&[(0.0, 0.0), (3.0, 3.0), (6.0, 0.0)], "NONE");
        for k in &mut fc.keyframe_points {
            k.handle_left_type = HandleType::Vector;
            k.handle_right_type = HandleType::Vector;
        }
        recalc(&mut fc);
        let k = &fc.keyframe_points[1];
        assert_eq!(k.handle_left, [2.0, 2.0]);
        assert_eq!(k.handle_right, [4.0, 2.0]);
    }

    #[test]
    fn solver_matches_direct_solution() {
        // 2x + y = 3, x + 2y + z = 4, y + 2z = 3 → (1, 1, 1).
        let mut x = [0.0; 3];
        assert!(tridiagonal_solve(
            &[0.0, 1.0, 1.0],
            &[2.0, 2.0, 2.0],
            &[1.0, 1.0, 0.0],
            &[3.0, 4.0, 3.0],
            &mut x
        ));
        for v in x {
            assert!((v - 1.0).abs() < 1e-6);
        }
        assert_eq!(increment_ulp(0.0), f32::from_bits(1));
        assert_eq!(decrement_ulp(1.0), f32::from_bits(1.0_f32.to_bits() - 1));
    }
}
