// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.deduplicate()` (`rna_FKeyframe_points_deduplicate` →
//! `BKE_fcurve_deduplicate_keys`, Blender 5.2.4): of sorted keys closer than 0.01 frame the
//! last one wins (at the earlier time unless it sits on a whole frame); no handle
//! recalculation.

use frap_anim::fcurve::keys;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::with_fcurve;

#[rna_func(srna = "FCurveKeyframePoints", func = "deduplicate")]
fn f_curve_keyframe_points_deduplicate(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    with_fcurve(h, this, |fc, _| {
        keys::deduplicate(fc);
        Ok(Value::None)
    })
}
