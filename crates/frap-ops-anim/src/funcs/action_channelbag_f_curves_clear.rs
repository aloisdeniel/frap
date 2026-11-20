// SPDX-License-Identifier: GPL-3.0-or-later
//! `channelbag.fcurves.clear()` (`rna_Channelbag_fcurve_clear` → `Channelbag::fcurves_clear`,
//! Blender 5.2.4): every F-curve; the groups stay, empty.

use frap_anim::rna::action::{Loc, bag_mut};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{internal, with_action};

#[rna_func(srna = "ActionChannelbagFCurves", func = "clear")]
fn action_channelbag_f_curves_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    with_action(h, this, |act, loc| {
        let Loc::Bag(l, s, b) = loc else {
            return Err(internal("fcurves.clear() outside a channelbag"));
        };
        if let Some(bag) = bag_mut(act, l, s, b) {
            bag.fcurves_clear();
        }
        Ok(Value::None)
    })
}
