// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.fcurves.ensure(data_path, index=0, group_name="")`
//! (`rna_Channelbag_fcurve_ensure` → `Channelbag::fcurve_ensure`, Blender 5.2.4): the existing
//! F-curve (its group unchanged), or a new one as `fcurves.new` makes it. An empty path raises
//! `RuntimeError: Error: F-Curve data path empty, invalid argument`.

use frap_anim::rna::action::{Loc, bag_mut};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{
    action_of, arg_int, arg_str, internal, loc_ptr, report_error, with_action,
};

#[rna_func(srna = "ActionChannelbagFCurves", func = "ensure")]
fn action_channelbag_f_curves_ensure(
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
            return Err(internal("fcurves.ensure() outside a channelbag"));
        };
        let bag = bag_mut(act, l, s, b).ok_or_else(|| internal("no channelbag"))?;
        let group = (!group.is_empty()).then_some(group.as_str());
        let f = bag.fcurve_ensure(&data_path, index, group);
        Ok(loc_ptr(a, act, Loc::FCurve(l, s, b, f)))
    })
}
