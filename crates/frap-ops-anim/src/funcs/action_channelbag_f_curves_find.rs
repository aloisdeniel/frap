// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.fcurves.find(data_path, index=0)` (`rna_Channelbag_fcurve_find`, Blender
//! 5.2.4): the F-curve or `None`. An empty path raises `RuntimeError: Error: F-Curve data path
//! empty, invalid argument`.

use frap_anim::rna::action::{Loc, bag};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{arg_int, arg_str, internal, loc_ptr, removed, report_error, target};

#[rna_func(srna = "ActionChannelbagFCurves", func = "find")]
fn action_channelbag_f_curves_find(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let data_path = arg_str(args, "data_path");
    let index = i32::try_from(arg_int(args, "index", 0)).unwrap_or(0);
    if data_path.is_empty() {
        return Err(report_error("F-Curve data path empty, invalid argument"));
    }
    let (a, loc) = target(h.main(), this)?;
    let Loc::Bag(l, s, b) = loc else {
        return Err(internal("fcurves.find() outside a channelbag"));
    };
    let act = h.main().actions.get(a).ok_or_else(|| removed("Action"))?;
    let found = bag(act, l, s, b).and_then(|x| x.fcurve_find(data_path, index));
    Ok(match found {
        Some(f) => loc_ptr(a, act, Loc::FCurve(l, s, b, f)),
        None => Value::None,
    })
}
