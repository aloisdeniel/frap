// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.remove(keyframe, fast=False)` (`rna_FKeyframe_points_remove` →
//! `BKE_fcurve_delete_key`, Blender 5.2.4): the key, then the handles recalculated unless
//! `fast`. A key of another F-curve raises `RuntimeError: Error: Keyframe not in F-Curve`.

use frap_anim::eval::recalc_handles;
use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_bool, arg_ptr, loc_arg, report_error, with_fcurve};

#[rna_func(srna = "FCurveKeyframePoints", func = "remove")]
fn f_curve_keyframe_points_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    let key = arg_ptr(args, "keyframe").and_then(|p| loc_arg(h.main(), a, p));
    let fast = arg_bool(args, "fast", false);
    with_fcurve(h, this, |fc, loc| {
        let Loc::FCurve(l, s, b, f) = loc else {
            return Err(report_error("Keyframe not in F-Curve"));
        };
        match key {
            Some(Loc::Key(kl, ks, kb, kf, k))
                if (kl, ks, kb, kf) == (l, s, b, f) && k < fc.keyframe_points.len() =>
            {
                fc.keyframe_points.remove(k);
                if !fast {
                    recalc_handles(fc);
                }
                Ok(Value::None)
            }
            _ => Err(report_error("Keyframe not in F-Curve")),
        }
    })
}
