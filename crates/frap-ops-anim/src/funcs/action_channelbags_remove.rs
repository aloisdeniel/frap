// SPDX-License-Identifier: GPL-3.0-or-later
//! `strip.channelbags.remove(channelbag)` (`rna_Channelbags_remove`, Blender 5.2.4): the
//! channelbag with its F-curves and groups. One of another strip raises
//! `RuntimeError: Error: This channelbag does not belong to this strip`.

use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_ptr, internal, loc_arg, report_error, with_action};

#[rna_func(srna = "ActionChannelbags", func = "remove")]
fn action_channelbags_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    let target = arg_ptr(args, "channelbag").and_then(|p| loc_arg(h.main(), a, p));
    with_action(h, this, |act, loc| {
        let Loc::Strip(l, s) = loc else {
            return Err(internal("channelbags.remove() outside a keyframe strip"));
        };
        match target {
            Some(Loc::Bag(bl, bs, b)) if (bl, bs) == (l, s) => {
                let Some(k) = act.layers[l].strips[s].keyframe_mut() else {
                    return Err(internal("not a keyframe strip"));
                };
                k.channelbags.remove(b);
                Ok(Value::None)
            }
            _ => Err(report_error(
                "This channelbag does not belong to this strip",
            )),
        }
    })
}
