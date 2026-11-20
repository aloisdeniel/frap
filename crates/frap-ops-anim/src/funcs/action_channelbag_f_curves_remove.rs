// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.fcurves.remove(fcurve)` (`rna_Channelbag_fcurve_remove` →
//! `Channelbag::fcurve_remove`, Blender 5.2.4): the F-curve; its group goes too when it was the
//! group's last F-curve. One of another channelbag raises `RuntimeError: Error: F-Curve not
//! found`.

use frap_anim::rna::action::{Loc, bag_mut};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_ptr, internal, loc_arg, report_error, with_action};

#[rna_func(srna = "ActionChannelbagFCurves", func = "remove")]
fn action_channelbag_f_curves_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    let target = arg_ptr(args, "fcurve").and_then(|p| loc_arg(h.main(), a, p));
    with_action(h, this, |act, loc| {
        let Loc::Bag(l, s, b) = loc else {
            return Err(internal("fcurves.remove() outside a channelbag"));
        };
        match target {
            Some(Loc::FCurve(fl, fs, fb, f)) if (fl, fs, fb) == (l, s, b) => {
                if let Some(bag) = bag_mut(act, l, s, b) {
                    bag.fcurve_remove(f);
                }
                Ok(Value::None)
            }
            _ => Err(report_error("F-Curve not found")),
        }
    })
}
