// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.sort()` (`rna_FKeyframe_points_sort` → `sort_time_fcurve`, Blender
//! 5.2.4): sorts the keys by time (stable bubble sort), swaps handles that crossed their key and
//! clamps the others to their side; no handle recalculation.

use frap_anim::fcurve::keys;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::with_fcurve;

#[rna_func(srna = "FCurveKeyframePoints", func = "sort")]
fn f_curve_keyframe_points_sort(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    with_fcurve(h, this, |fc, _| {
        keys::sort_time(fc);
        Ok(Value::None)
    })
}
