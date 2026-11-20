// SPDX-License-Identifier: GPL-3.0-or-later
//! `action.deselect_keys()` (`rna_Action_deselect_keys`, Blender 5.2.4): deselects every key
//! (control point and both handles) of every F-curve; F-curve selection is unchanged.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::with_action;

#[rna_func(srna = "Action", func = "deselect_keys")]
fn action_deselect_keys(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    with_action(h, this, |act, _| {
        act.deselect_keys();
        Ok(Value::None)
    })
}
