// SPDX-License-Identifier: GPL-3.0-or-later
//! `action.layers.remove(anim_layer)` (`rna_Action_layers_remove` → `Action::layer_remove`,
//! Blender 5.2.4): the layer with its strips. A layer of another action raises
//! `RuntimeError: Error: This layer does not belong to this Action`.

use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_ptr, loc_arg, report_error, with_action};

#[rna_func(srna = "ActionLayers", func = "remove")]
fn action_layers_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    let Some(Loc::Layer(l)) = arg_ptr(args, "anim_layer").and_then(|p| loc_arg(h.main(), a, p))
    else {
        return Err(report_error("This layer does not belong to this Action"));
    };
    with_action(h, this, |act, _| {
        act.layers.remove(l);
        Ok(Value::None)
    })
}
