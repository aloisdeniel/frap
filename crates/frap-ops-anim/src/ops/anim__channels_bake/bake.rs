// SPDX-License-Identifier: GPL-3.0-or-later
//! Baking one F-curve (`anim.channels_bake`): samples the curve at every `step` over a frame
//! range, removes the keys in the range (or all of them), merges the samples in and sets the
//! baked keys' interpolation.
//!
//! Ported from blender/source/blender/editors/animation/anim_channels_edit.cc
//! (`channels_bake_exec`, the per-curve part), blender/source/blender/animrig/intern/fcurve.cc
//! (`bake_fcurve`, `sample_fcurve_segment`, `remove_fcurve_key_range`) and
//! blender/source/blender/blenkernel/intern/fcurve.cc (`BKE_bezier_array_merge`,
//! `BKE_fcurve_delete_keys`), blender/source/blender/blenlib/intern/math_base_inline.cc
//! (`compare_ff_relative`), Blender 5.2.4, GPL-2.0-or-later.

use frap_anim::eval::curve::{BEZT_BINARYSEARCH_THRESH, binarysearch_index};
use frap_anim::eval::{evaluate_fcurve, recalc_handles};
use frap_anim::fcurve::keys::{KeyframeSettings, initialize_bezt};
use frap_anim::{FCurve, Interpolation, Keyframe};

/// `BakeCurveRemove`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Remove {
    /// `IN_RANGE`: the keys inside the baked range.
    InRange,
    /// `ALL`: every key.
    All,
}

/// `compare_ff_relative(a, b, max_diff, max_ulps)`.
fn compare_ff_relative(a: f32, b: f32, max_diff: f32, max_ulps: i32) -> bool {
    if (a - b).abs() <= max_diff {
        return true;
    }
    let (ua, ub) = (a.to_bits() as i32, b.to_bits() as i32);
    (a < 0.0) == (b < 0.0) && ua.wrapping_sub(ub).wrapping_abs() <= max_ulps
}

/// `BKE_fcurve_bezt_binarysearch_index` (0 for an empty curve, as Blender after its warning).
fn search(keys: &[Keyframe], frame: f32) -> (usize, bool) {
    if keys.is_empty() {
        return (0, false);
    }
    binarysearch_index(keys, frame)
}

/// `remove_fcurve_key_range`
fn remove_range(fc: &mut FCurve, range: [i32; 2], mode: Remove) {
    match mode {
        Remove::All => fc.keyframe_points.clear(),
        Remove::InRange => {
            let (start, _) = search(&fc.keyframe_points, range[0] as f32);
            let (mut end, replace) = search(&fc.keyframe_points, range[1] as f32);
            if replace {
                end += 1;
            }
            if end > start {
                let end = end.min(fc.keyframe_points.len());
                fc.keyframe_points.drain(start.min(end)..end);
            }
        }
    }
}

/// `BKE_bezier_array_merge(a, b)`: keys of `a` replace keys of `b` on (nearly) the same frame.
fn merge(a: Vec<Keyframe>, b: Vec<Keyframe>) -> Vec<Keyframe> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut ia, mut ib) = (0, 0);
    while ia < a.len() || ib < b.len() {
        if ia >= a.len() {
            out.extend_from_slice(&b[ib..]);
            break;
        }
        if ib >= b.len() {
            out.extend_from_slice(&a[ia..]);
            break;
        }
        let (fa, fb) = (a[ia].co[0], b[ib].co[0]);
        if compare_ff_relative(fa, fb, BEZT_BINARYSEARCH_THRESH, 32) {
            out.push(a[ia].clone());
            ia += 1;
            ib += 1;
        } else if fa < fb {
            out.push(a[ia].clone());
            ia += 1;
        } else {
            out.push(b[ib].clone());
            ib += 1;
        }
    }
    out
}

/// `bake_fcurve(fcu, range, step, remove_existing)`.
fn bake_fcurve(fc: &mut FCurve, range: [i32; 2], step: f32, remove: Remove) {
    // `const int sample_count = (range[1] - range[0]) / step + 1;`
    let sample_count = ((range[1] - range[0]) as f32 / step + 1.0) as i32;
    let sample_count = usize::try_from(sample_count).unwrap_or(0);
    let sample_rate = 1.0f32 / step;
    let start = range[0] as f32;
    let samples: Vec<f32> = (0..sample_count)
        .map(|i| evaluate_fcurve(fc, start + (i as f32 / sample_rate)))
        .collect();
    remove_range(fc, range, remove);
    let settings = KeyframeSettings::default();
    let baked: Vec<Keyframe> = samples
        .iter()
        .enumerate()
        .map(|(i, &v)| initialize_bezt([range[0] as f32 + i as f32 * step, v], &settings, fc))
        .collect();
    let existing = std::mem::take(&mut fc.keyframe_points);
    fc.keyframe_points = merge(baked, existing);
    recalc_handles(fc);
}

/// The per-curve body of `channels_bake_exec`: bakes `fc` over `range` and gives the keys in
/// the range `ipo`, the key that ends the baked segment keeping the interpolation the curve
/// had there. A curve without keys is left alone.
pub(crate) fn bake(fc: &mut FCurve, range: [i32; 2], step: f32, remove: Remove, ipo: Interpolation) {
    if fc.keyframe_points.is_empty() {
        return;
    }
    let (last_index, _) = search(&fc.keyframe_points, range[1] as f32);
    let n = fc.keyframe_points.len();
    let segment_end_ipo = fc.keyframe_points[last_index.min(n - 1)].interpolation;
    bake_fcurve(fc, range, step, remove);
    let len = fc.keyframe_points.len();
    for i in 0..len {
        let x = fc.keyframe_points[i].co[0];
        if x < range[0] as f32 {
            continue;
        }
        if x > range[1] as f32 {
            fc.keyframe_points[i.saturating_sub(1)].interpolation = segment_end_ipo;
            break;
        }
        fc.keyframe_points[i].interpolation = ipo;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(keys: &[(f32, f32)]) -> FCurve {
        let mut fc = FCurve::new("location", 0);
        fc.keyframe_points = keys.iter().map(|&(x, y)| Keyframe::new(x, y)).collect();
        recalc_handles(&mut fc);
        fc
    }

    #[test]
    fn bakes_every_step_and_keeps_outside_keys() {
        let mut fc = curve(&[(1.0, 0.0), (10.0, 9.0), (20.0, 0.0)]);
        bake(&mut fc, [2, 6], 2.0, Remove::InRange, Interpolation::Linear);
        let xs: Vec<f32> = fc.keyframe_points.iter().map(|k| k.co[0]).collect();
        assert_eq!(xs, vec![1.0, 2.0, 4.0, 6.0, 10.0, 20.0]);
        // The baked keys take LINEAR; the last baked one keeps the BEZIER the curve had.
        let ipo: Vec<Interpolation> = fc.keyframe_points.iter().map(|k| k.interpolation).collect();
        assert_eq!(ipo[1], Interpolation::Linear);
        assert_eq!(ipo[2], Interpolation::Linear);
        assert_eq!(ipo[3], Interpolation::Bezier);
    }

    #[test]
    fn remove_all_leaves_only_the_samples() {
        let mut fc = curve(&[(1.0, 0.0), (10.0, 9.0)]);
        bake(&mut fc, [3, 5], 1.0, Remove::All, Interpolation::Constant);
        let xs: Vec<f32> = fc.keyframe_points.iter().map(|k| k.co[0]).collect();
        assert_eq!(xs, vec![3.0, 4.0, 5.0]);
        assert!(
            fc.keyframe_points
                .iter()
                .all(|k| k.interpolation == Interpolation::Constant)
        );
    }

    #[test]
    fn relative_float_compare() {
        assert!(compare_ff_relative(1.0, 1.005, 0.01, 32));
        assert!(!compare_ff_relative(1.0, 1.5, 0.01, 32));
        assert!(compare_ff_relative(1000.0, 1000.0 + 32.0 * 6.1e-5, 0.01, 32));
    }
}
