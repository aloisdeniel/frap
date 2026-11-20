// SPDX-License-Identifier: GPL-3.0-or-later
//! `action.layers.new(name)` (`rna_Action_layers_new` → `Action::layer_add`, Blender 5.2.4):
//! the action's layer (full influence, REPLACE). Blender 5.2 allows one layer per action: a
//! second raises `RuntimeError: Error: An Action may not have more than one layer` (verified).
//! The name is kept as given (layers are not renamed to be unique).

use frap_anim::ActionLayer;
use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_str, loc_ptr, report_error, with_action};

#[rna_func(srna = "ActionLayers", func = "new")]
fn action_layers_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let a = action_of(this)?;
    with_action(h, this, |act, _| {
        if !act.layers.is_empty() {
            return Err(report_error("An Action may not have more than one layer"));
        }
        act.layers.push(ActionLayer::new(name));
        Ok(loc_ptr(a, act, Loc::Layer(act.layers.len() - 1)))
    })
}
