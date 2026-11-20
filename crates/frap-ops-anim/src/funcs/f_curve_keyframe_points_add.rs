// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.add(count)` (`rna_FKeyframe_points_add` → `ED_keyframes_add`,
//! Blender 5.2.4): appends `count` keys at (0, 0) with their handles at (0, 0), BEZIER,
//! AUTO_CLAMPED, selected, `back`/`amplitude`/`period` 0, without sorting or recalculating the
//! handles (scripts set `co` and call `fcurve.update()`). `count <= 0` adds nothing.

use frap_anim::fcurve::keys;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{arg_int, with_fcurve};

#[rna_func(srna = "FCurveKeyframePoints", func = "add")]
fn f_curve_keyframe_points_add(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let count = arg_int(args, "count", 1);
    if count <= 0 {
        return Ok(Value::None);
    }
    with_fcurve(h, this, |fc, _| {
        keys::add_zeroed(fc, usize::try_from(count).unwrap_or(0));
        Ok(Value::None)
    })
}
