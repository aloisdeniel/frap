// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.clear()` (`rna_FKeyframe_points_clear` →
//! `BKE_fcurve_delete_keys_all`, Blender 5.2.4): every key.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::with_fcurve;

#[rna_func(srna = "FCurveKeyframePoints", func = "clear")]
fn f_curve_keyframe_points_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    with_fcurve(h, this, |fc, _| {
        fc.keyframe_points.clear();
        Ok(Value::None)
    })
}
