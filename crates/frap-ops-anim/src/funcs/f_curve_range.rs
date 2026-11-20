// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.range() -> (start, end)` (`rna_FCurve_range` → `BKE_fcurve_calc_range`, Blender
//! 5.2.4): the times of the first and last key, `(0, 0)` without keys.

use frap_anim::fcurve::keys;
use frap_rna::{ArrayShape, MathTag, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::with_fcurve;

#[rna_func(srna = "FCurve", func = "range")]
fn f_curve_range(h: &mut dyn Host, this: &RnaPtr, _args: &mut FuncArgs) -> Result<Value, RnaError> {
    with_fcurve(h, this, |fc, _| {
        let r = keys::range(fc).unwrap_or([0.0, 0.0]);
        Ok(Value::FloatArray {
            data: r.into_iter().collect(),
            shape: ArrayShape::Fixed(2),
            tag: MathTag::None,
        })
    })
}
