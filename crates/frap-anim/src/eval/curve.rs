// SPDX-License-Identifier: GPL-3.0-or-later
//! F-curve sampling: `evaluate_fcurve` → `fcurve_eval_keyframes` (extrapolation, binary
//! search, the 13 interpolations with their easings, Bézier segments through
//! `BKE_fcurve_correct_bezpart` / `findzero` / `berekeny`).
//!
//! Ported from blender/source/blender/blenkernel/intern/fcurve.cc (Blender 5.2.4,
//! GPL-2.0-or-later), operation order kept so `f32` results match.

use super::easing as e;
use crate::fcurve::{Easing, Extrapolation, FCurve, Interpolation, Keyframe};

/// `SMALL` in `fcurve.cc`.
const SMALL: f64 = -1.0e-10;
/// `BEZT_BINARYSEARCH_THRESH`.
pub const BEZT_BINARYSEARCH_THRESH: f32 = 0.01;

/// `IS_EQT(a, b, c)`.
fn is_eqt(a: f32, b: f32, c: f32) -> bool {
    if a > b { (a - b) <= c } else { (b - a) <= c }
}

/// `IS_EQF(a, b)`.
pub(crate) fn is_eqf(a: f32, b: f32) -> bool {
    (a - b).abs() < f32::EPSILON
}

/// `BKE_fcurve_bezt_binarysearch_index_ex`: the index of the key at `frame` (`replace`) or where
/// a key at `frame` would be inserted.
pub fn binarysearch_index_ex(keys: &[Keyframe], frame: f32, threshold: f32) -> (usize, bool) {
    if keys.is_empty() {
        return (0, false);
    }
    let first = keys[0].co[0];
    if is_eqt(frame, first, threshold) {
        return (0, true);
    }
    if frame < first {
        return (0, false);
    }
    let last = keys[keys.len() - 1].co[0];
    if is_eqt(frame, last, threshold) {
        return (keys.len() - 1, true);
    }
    if frame > last {
        return (keys.len(), false);
    }
    let (mut start, mut end) = (0_i64, keys.len() as i64);
    while start <= end {
        let mid = start + (end - start) / 2;
        let midfra = keys[mid as usize].co[0];
        if is_eqt(frame, midfra, threshold) {
            return (mid as usize, true);
        }
        if frame > midfra {
            start = mid + 1;
        } else {
            end = mid - 1;
        }
    }
    (start as usize, false)
}

/// `BKE_fcurve_bezt_binarysearch_index` (threshold [`BEZT_BINARYSEARCH_THRESH`]).
pub fn binarysearch_index(keys: &[Keyframe], frame: f32) -> (usize, bool) {
    binarysearch_index_ex(keys, frame, BEZT_BINARYSEARCH_THRESH)
}

/// `BKE_fcurve_correct_bezpart`: keeps the handles of a segment from exceeding the adjacent
/// key's time, so the curve cannot loop back in time.
pub fn correct_bezpart(v1: [f32; 2], v2: &mut [f32; 2], v3: &mut [f32; 2], v4: [f32; 2]) {
    let h1 = [v1[0] - v2[0], v1[1] - v2[1]];
    let h2 = [v4[0] - v3[0], v4[1] - v3[1]];
    let len = v4[0] - v1[0];
    let len1 = h1[0].abs();
    let len2 = h2[0].abs();
    if (len1 + len2) == 0.0 {
        return;
    }
    if len1 > len {
        let fac = len / len1;
        v2[0] = v1[0] - fac * h1[0];
        v2[1] = v1[1] - fac * h1[1];
    }
    if len2 > len {
        let fac = len / len2;
        v3[0] = v4[0] - fac * h2[0];
        v3[1] = v4[1] - fac * h2[1];
    }
}

/// `sqrt3d`
fn sqrt3d(d: f64) -> f64 {
    if d == 0.0 {
        return 0.0;
    }
    if d < 0.0 {
        return -libm::exp(libm::log(-d) / 3.0);
    }
    libm::exp(libm::log(d) / 3.0)
}

/// Is a root of [`solve_cubic`] inside the segment (`SMALL <= x <= 1.000001`)?
fn in_unit(x: f32) -> bool {
    x >= SMALL as f32 && x <= 1.000_001
}

/// `solve_cubic`: roots of `c0 + c1 x + c2 x² + c3 x³` in `[SMALL, 1.000001]`, written to `o`;
/// returns how many.
fn solve_cubic(c0: f64, c1: f64, c2: f64, c3: f64, o: &mut [f32; 3]) -> usize {
    let mut nr = 0;
    if c3 != 0.0 {
        let mut a = c2 / c3;
        let b = c1 / c3;
        let c = c0 / c3;
        a /= 3.0;
        let mut p = b / 3.0 - a * a;
        let mut q = (2.0 * a * a * a - a * b + c) / 2.0;
        let d = q * q + p * p * p;
        if d > 0.0 {
            let t = d.sqrt();
            o[0] = (sqrt3d(-q + t) + sqrt3d(-q - t) - a) as f32;
            return usize::from(in_unit(o[0]));
        }
        if d == 0.0 {
            let t = sqrt3d(-q);
            o[0] = (2.0 * t - a) as f32;
            if in_unit(o[0]) {
                nr += 1;
            }
            o[nr] = (-t - a) as f32;
            if in_unit(o[nr]) {
                return nr + 1;
            }
            return nr;
        }
        let phi = libm::acos(-q / (-(p * p * p)).sqrt());
        let t = (-p).sqrt();
        p = libm::cos(phi / 3.0);
        q = (3.0 - 3.0 * p * p).sqrt();
        o[0] = (2.0 * t * p - a) as f32;
        if in_unit(o[0]) {
            nr += 1;
        }
        o[nr] = (-t * (p + q) - a) as f32;
        if in_unit(o[nr]) {
            nr += 1;
        }
        o[nr] = (-t * (p - q) - a) as f32;
        if in_unit(o[nr]) {
            return nr + 1;
        }
        return nr;
    }
    let a = c2;
    let b = c1;
    let c = c0;
    if a != 0.0 {
        let mut p = b * b - 4.0 * a * c;
        if p > 0.0 {
            p = p.sqrt();
            o[0] = ((-b - p) / (2.0 * a)) as f32;
            if in_unit(o[0]) {
                nr += 1;
            }
            o[nr] = ((-b + p) / (2.0 * a)) as f32;
            if in_unit(o[nr]) {
                return nr + 1;
            }
            return nr;
        }
        if p == 0.0 {
            o[0] = (-b / (2.0 * a)) as f32;
            if in_unit(o[0]) {
                return 1;
            }
        }
        return 0;
    }
    if b != 0.0 {
        o[0] = (-c / b) as f32;
        return usize::from(in_unit(o[0]));
    }
    if c == 0.0 {
        o[0] = 0.0;
        return 1;
    }
    0
}

/// `findzero`: the parameters where the Bézier `q0..q3` (time components) reaches `x`.
pub(crate) fn findzero(x: f32, q0: f32, q1: f32, q2: f32, q3: f32, o: &mut [f32; 3]) -> usize {
    let c0 = f64::from(q0 - x);
    let c1 = f64::from(3.0_f32 * (q1 - q0));
    let c2 = f64::from(3.0_f32 * (q0 - 2.0 * q1 + q2));
    let c3 = f64::from(q3 - q0 + 3.0 * (q1 - q2));
    solve_cubic(c0, c1, c2, c3, o)
}

/// `berekeny` for one parameter: the value component of the Bézier at `t`.
fn berekeny(f1: f32, f2: f32, f3: f32, f4: f32, t: f32) -> f32 {
    let c0 = f1;
    let c1 = 3.0 * (f2 - f1);
    let c2 = 3.0 * (f1 - 2.0 * f2 + f3);
    let c3 = f4 - f1 + 3.0 * (f2 - f3);
    c0 + t * c1 + t * t * c2 + t * t * t * c3
}

/// `fcurve_eval_keyframes_extrapolate`
fn extrapolate(
    fc: &FCurve,
    keys: &[Keyframe],
    evaltime: f32,
    endpoint: usize,
    toward_neighbor: isize,
) -> f32 {
    let end = &keys[endpoint];
    if end.interpolation == Interpolation::Constant
        || fc.extrapolation == Extrapolation::Constant
        || fc.discrete_values
    {
        return end.co[1];
    }
    if end.interpolation == Interpolation::Linear {
        if keys.len() == 1 {
            return end.co[1];
        }
        let neighbor = &keys[(endpoint as isize + toward_neighbor) as usize];
        let dx = end.co[0] - evaltime;
        let mut fac = neighbor.co[0] - end.co[0];
        if fac == 0.0 {
            return end.co[1];
        }
        fac = (neighbor.co[1] - end.co[1]) / fac;
        return end.co[1] - (fac * dx);
    }
    // The gradient of the endpoint's outer handle.
    let handle = if toward_neighbor > 0 {
        end.handle_left
    } else {
        end.handle_right
    };
    let dx = end.co[0] - evaltime;
    let mut fac = end.co[0] - handle[0];
    if fac == 0.0 {
        return end.co[1];
    }
    fac = (end.co[1] - handle[1]) / fac;
    end.co[1] - (fac * dx)
}

/// `fcurve_eval_keyframes_interpolate`
fn interpolate(fc: &FCurve, keys: &[Keyframe], evaltime: f32) -> f32 {
    let eps = 1.0e-8_f32;
    let (a, exact) = binarysearch_index_ex(keys, evaltime, 0.0001);
    let bezt = &keys[a];
    if exact {
        return bezt.co[1];
    }
    let prevbezt = if a > 0 { &keys[a - 1] } else { bezt };
    if (bezt.co[0] - evaltime).abs() < eps {
        return bezt.co[1];
    }
    if evaltime < prevbezt.co[0] || bezt.co[0] < evaltime {
        return 0.0;
    }
    let begin = prevbezt.co[1];
    let change = bezt.co[1] - prevbezt.co[1];
    let duration = bezt.co[0] - prevbezt.co[0];
    let time = evaltime - prevbezt.co[0];
    let amplitude = prevbezt.amplitude;
    let period = prevbezt.period;

    if prevbezt.interpolation == Interpolation::Constant || fc.discrete_values || duration == 0.0 {
        return prevbezt.co[1];
    }
    let easing = prevbezt.easing;
    use Easing::{EaseIn, EaseInOut, EaseOut};
    match prevbezt.interpolation {
        Interpolation::Bezier => {
            let v1 = prevbezt.co;
            let mut v2 = prevbezt.handle_right;
            let mut v3 = bezt.handle_left;
            let v4 = bezt.co;
            if (v1[1] - v4[1]).abs() < f32::EPSILON
                && (v2[1] - v3[1]).abs() < f32::EPSILON
                && (v3[1] - v4[1]).abs() < f32::EPSILON
            {
                // All handles flat at the same value.
                return v1[1];
            }
            correct_bezpart(v1, &mut v2, &mut v3, v4);
            let mut opl = [0.0_f32; 3];
            if findzero(evaltime, v1[0], v2[0], v3[0], v4[0], &mut opl) == 0 {
                return 0.0;
            }
            berekeny(v1[1], v2[1], v3[1], v4[1], opl[0])
        }
        Interpolation::Linear => e::linear_ease(time, begin, change, duration),
        Interpolation::Back => {
            let b = prevbezt.back;
            match easing {
                EaseIn => e::back_ease_in(time, begin, change, duration, b),
                EaseInOut => e::back_ease_in_out(time, begin, change, duration, b),
                // EASE_OUT and AUTO (same as ease out).
                _ => e::back_ease_out(time, begin, change, duration, b),
            }
        }
        Interpolation::Bounce => match easing {
            EaseIn => e::bounce_ease_in(time, begin, change, duration),
            EaseInOut => e::bounce_ease_in_out(time, begin, change, duration),
            _ => e::bounce_ease_out(time, begin, change, duration),
        },
        Interpolation::Circ => match easing {
            EaseOut => e::circ_ease_out(time, begin, change, duration),
            EaseInOut => e::circ_ease_in_out(time, begin, change, duration),
            // EASE_IN and AUTO (same as ease in).
            _ => e::circ_ease_in(time, begin, change, duration),
        },
        Interpolation::Cubic => match easing {
            EaseOut => e::cubic_ease_out(time, begin, change, duration),
            EaseInOut => e::cubic_ease_in_out(time, begin, change, duration),
            _ => e::cubic_ease_in(time, begin, change, duration),
        },
        Interpolation::Elastic => match easing {
            EaseIn => e::elastic_ease_in(time, begin, change, duration, amplitude, period),
            EaseInOut => e::elastic_ease_in_out(time, begin, change, duration, amplitude, period),
            _ => e::elastic_ease_out(time, begin, change, duration, amplitude, period),
        },
        Interpolation::Expo => match easing {
            EaseOut => e::expo_ease_out(time, begin, change, duration),
            EaseInOut => e::expo_ease_in_out(time, begin, change, duration),
            _ => e::expo_ease_in(time, begin, change, duration),
        },
        Interpolation::Quad => match easing {
            EaseOut => e::quad_ease_out(time, begin, change, duration),
            EaseInOut => e::quad_ease_in_out(time, begin, change, duration),
            _ => e::quad_ease_in(time, begin, change, duration),
        },
        Interpolation::Quart => match easing {
            EaseOut => e::quart_ease_out(time, begin, change, duration),
            EaseInOut => e::quart_ease_in_out(time, begin, change, duration),
            _ => e::quart_ease_in(time, begin, change, duration),
        },
        Interpolation::Quint => match easing {
            EaseOut => e::quint_ease_out(time, begin, change, duration),
            EaseInOut => e::quint_ease_in_out(time, begin, change, duration),
            _ => e::quint_ease_in(time, begin, change, duration),
        },
        Interpolation::Sine => match easing {
            EaseOut => e::sine_ease_out(time, begin, change, duration),
            EaseInOut => e::sine_ease_in_out(time, begin, change, duration),
            _ => e::sine_ease_in(time, begin, change, duration),
        },
        Interpolation::Constant => prevbezt.co[1],
    }
}

/// `fcurve_eval_keyframes`
fn eval_keyframes(fc: &FCurve, keys: &[Keyframe], evaltime: f32) -> f32 {
    if evaltime <= keys[0].co[0] {
        return extrapolate(fc, keys, evaltime, 0, 1);
    }
    let last = keys.len() - 1;
    if keys[last].co[0] <= evaltime {
        return extrapolate(fc, keys, evaltime, last, -1);
    }
    interpolate(fc, keys, evaltime)
}

/// `evaluate_fcurve_ex` without F-modifiers: the keys (0 without any), rounded for integer
/// curves (`FCURVE_INT_VALUES`).
pub fn evaluate(fc: &FCurve, evaltime: f32) -> f32 {
    let mut cvalue = 0.0;
    if !fc.keyframe_points.is_empty() {
        cvalue = eval_keyframes(fc, &fc.keyframe_points, evaltime);
    }
    if fc.int_values {
        cvalue = (cvalue + 0.5).floor();
    }
    cvalue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(f: f32, v: f32, ipo: Interpolation) -> Keyframe {
        Keyframe {
            interpolation: ipo,
            ..Keyframe::new(f, v)
        }
    }

    #[test]
    fn binary_search() {
        let k: Vec<_> = [1.0, 5.0, 9.0, 20.0]
            .iter()
            .map(|f| key(*f, 0.0, Interpolation::Linear))
            .collect();
        assert_eq!(binarysearch_index(&k, 0.0), (0, false));
        assert_eq!(binarysearch_index(&k, 1.005), (0, true));
        assert_eq!(binarysearch_index(&k, 6.0), (2, false));
        assert_eq!(binarysearch_index(&k, 9.0), (2, true));
        assert_eq!(binarysearch_index(&k, 25.0), (4, false));
    }

    #[test]
    fn linear_and_constant() {
        let mut fc = FCurve::new("location", 0);
        fc.keyframe_points = vec![
            key(1.0, 0.0, Interpolation::Linear),
            key(11.0, 10.0, Interpolation::Constant),
        ];
        assert_eq!(evaluate(&fc, 6.0), 5.0);
        assert_eq!(evaluate(&fc, -5.0), 0.0);
        assert_eq!(evaluate(&fc, 30.0), 10.0);
        fc.extrapolation = Extrapolation::Linear;
        // The first key is LINEAR: extrapolates along the first segment.
        assert_eq!(evaluate(&fc, -1.0), -2.0);
        // The last key is CONSTANT: holds.
        assert_eq!(evaluate(&fc, 30.0), 10.0);
        fc.int_values = true;
        assert_eq!(evaluate(&fc, 6.4), 5.0);
        assert_eq!(evaluate(&fc, 6.6), 6.0);
        fc.discrete_values = true;
        assert_eq!(evaluate(&fc, 6.6), 0.0);
        assert_eq!(evaluate(&FCurve::new("x", 0), 3.0), 0.0);
    }

    #[test]
    fn cubic_roots() {
        let mut o = [0.0; 3];
        // A linear-in-time Bézier: x(t) = 3t on [0, 3], so x = 1.5 at t = 0.5.
        assert_eq!(findzero(1.5, 0.0, 1.0, 2.0, 3.0, &mut o), 1);
        assert!((o[0] - 0.5).abs() < 1e-6);
    }
}
