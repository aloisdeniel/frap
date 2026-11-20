// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.evaluate(frame) -> float` (`evaluate_fcurve`, Blender 5.2.4): the curve's value at
//! `frame` (interpolation, easing, extrapolation; 0 without keys; whole numbers for integer
//! curves). frap's F-curves have no F-modifiers or drivers.

use frap_anim::eval::evaluate_fcurve;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{arg_f32, with_fcurve};

#[rna_func(srna = "FCurve", func = "evaluate")]
fn f_curve_evaluate(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let frame = arg_f32(args, "frame", 1.0);
    with_fcurve(h, this, |fc, _| Ok(Value::Float(evaluate_fcurve(fc, frame))))
}
