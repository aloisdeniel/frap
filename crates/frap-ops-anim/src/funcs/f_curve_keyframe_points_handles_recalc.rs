// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.handles_recalc()` (`rna_FKeyframe_points_handles_recalc` →
//! `BKE_fcurve_handles_recalc`, Blender 5.2.4): recomputes AUTO, AUTO_CLAMPED and VECTOR
//! handles and their ALIGNED partners, with the curve's smoothing.

use frap_anim::eval::recalc_handles;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::with_fcurve;

#[rna_func(srna = "FCurveKeyframePoints", func = "handles_recalc")]
fn f_curve_keyframe_points_handles_recalc(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    with_fcurve(h, this, |fc, _| {
        recalc_handles(fc);
        Ok(Value::None)
    })
}
