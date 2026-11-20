// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.update()` (`rna_FCurve_update_data_ex`, Blender 5.2.4): sorts the keys by time,
//! merges keys closer than 0.01 frame (the last one wins), recalculates the handles.

use frap_anim::fcurve::keys;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::with_fcurve;

#[rna_func(srna = "FCurve", func = "update")]
fn f_curve_update(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    with_fcurve(h, this, |fc, _| {
        keys::update(fc);
        Ok(Value::None)
    })
}
