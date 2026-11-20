// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.fcurves.new(data_path, index=0, group_name="")` (`rna_Channelbag_fcurve_new` →
//! `Channelbag::fcurve_create_unique`, Blender 5.2.4): an empty F-curve (selected, the
//! preferences' `CONT_ACCEL` smoothing); with a group name it goes at the end of that group,
//! created when missing. An existing F-curve raises `RuntimeError: Error: F-Curve
//! 'location[0]' already exists in this channelbag`, an empty path `Error: F-Curve data path
//! empty, invalid argument` (verified on 5.2.4).

use frap_anim::rna::action::{Loc, bag_mut};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{
    action_of, arg_int, arg_str, internal, loc_ptr, report_error, with_action,
};

#[rna_func(srna = "ActionChannelbagFCurves", func = "new")]
fn action_channelbag_f_curves_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let data_path = arg_str(args, "data_path").to_owned();
    let index = i32::try_from(arg_int(args, "index", 0)).unwrap_or(0);
    let group = arg_str(args, "group_name").to_owned();
    if data_path.is_empty() {
        return Err(report_error("F-Curve data path empty, invalid argument"));
    }
    let a = action_of(this)?;
    with_action(h, this, |act, loc| {
        let Loc::Bag(l, s, b) = loc else {
            return Err(internal("fcurves.new() outside a channelbag"));
        };
        let bag = bag_mut(act, l, s, b).ok_or_else(|| internal("no channelbag"))?;
        let group = (!group.is_empty()).then_some(group.as_str());
        let Some(f) = bag.fcurve_create_unique(&data_path, index, group) else {
            return Err(report_error(format!(
                "F-Curve '{data_path}[{index}]' already exists in this channelbag"
            )));
        };
        Ok(loc_ptr(a, act, Loc::FCurve(l, s, b, f)))
    })
}
