// SPDX-License-Identifier: GPL-3.0-or-later
//! `layer.strips.remove(anim_strip)` (`rna_ActionStrips_remove` → `Layer::strip_remove`,
//! Blender 5.2.4): the strip with its channelbags. A strip of another layer raises
//! `RuntimeError: Error: This strip does not belong to this layer`.

use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_ptr, internal, loc_arg, report_error, with_action};

#[rna_func(srna = "ActionStrips", func = "remove")]
fn action_strips_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    let strip = arg_ptr(args, "anim_strip").and_then(|p| loc_arg(h.main(), a, p));
    with_action(h, this, |act, loc| {
        let Loc::Layer(l) = loc else {
            return Err(internal("strips.remove() outside a layer"));
        };
        match strip {
            Some(Loc::Strip(sl, s)) if sl == l => {
                act.layers[l].strips.remove(s);
                Ok(Value::None)
            }
            _ => Err(report_error("This strip does not belong to this layer")),
        }
    })
}
