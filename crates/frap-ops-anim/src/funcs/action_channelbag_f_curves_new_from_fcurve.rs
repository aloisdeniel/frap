// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.fcurves.new_from_fcurve(source, data_path="")`
//! (`rna_Channelbag_fcurve_new_from_fcurve`, Blender 5.2.4): a copy of `source` (keys and
//! settings), under `data_path` when given, appended ungrouped. An F-curve already at that
//! path and index raises `RuntimeError: Error: F-Curve 'location[0]' already exists in this
//! channelbag`. The source may be in any action.

use frap_anim::rna::action::{Loc, bag, bag_mut, locate};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{
    action_of, arg_ptr, arg_str, internal, loc_ptr, removed, report_error, with_action,
};

#[rna_func(srna = "ActionChannelbagFCurves", func = "new_from_fcurve")]
fn action_channelbag_f_curves_new_from_fcurve(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let src = arg_ptr(args, "source").ok_or_else(|| internal("no source F-curve"))?;
    let sa = action_of(src)?;
    let main = h.main();
    let sact = main.actions.get(sa).ok_or_else(|| removed("Action"))?;
    let Some((Loc::FCurve(l, s, b, f), None)) = locate(sact, &src.path) else {
        return Err(removed("FCurve"));
    };
    let mut fc = bag(sact, l, s, b)
        .and_then(|x| x.fcurves.get(f))
        .cloned()
        .ok_or_else(|| removed("FCurve"))?;
    let path = arg_str(args, "data_path");
    if !path.is_empty() {
        fc.data_path = path.into();
    }
    let a = action_of(this)?;
    with_action(h, this, |act, loc| {
        let Loc::Bag(l, s, b) = loc else {
            return Err(internal("fcurves.new_from_fcurve() outside a channelbag"));
        };
        let bag = bag_mut(act, l, s, b).ok_or_else(|| internal("no channelbag"))?;
        if bag.fcurve_find(&fc.data_path, fc.array_index).is_some() {
            return Err(report_error(format!(
                "F-Curve '{}[{}]' already exists in this channelbag",
                fc.data_path, fc.array_index
            )));
        }
        let f = bag.fcurve_append(fc);
        Ok(loc_ptr(a, act, Loc::FCurve(l, s, b, f)))
    })
}
