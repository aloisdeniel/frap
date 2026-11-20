// SPDX-License-Identifier: GPL-3.0-or-later
//! `strip.channelbag(slot, ensure=False)` (`rna_ActionStrip_channelbag`, Blender 5.2.4): the
//! channelbag of `slot` in this keyframe strip, `None` when there is none, appended when
//! `ensure=True`. `slot=None` raises `RuntimeError: Error: Cannot return channelbag when slot
//! is None`. Blender takes the slot's handle as is: a slot of another action addresses this
//! action's slot with the same handle.

use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{
    action_of, arg_bool, arg_ptr, internal, loc_ptr, report_error, slot_arg, with_action,
};

#[rna_func(srna = "ActionKeyframeStrip", func = "channelbag")]
fn action_keyframe_strip_channelbag(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let Some((_, handle)) = arg_ptr(args, "slot").and_then(|p| slot_arg(h.main(), p)) else {
        return Err(report_error("Cannot return channelbag when slot is None"));
    };
    let ensure = arg_bool(args, "ensure", false);
    let a = action_of(this)?;
    with_action(h, this, |act, loc| {
        let Loc::Strip(l, s) = loc else {
            return Err(internal("channelbag() outside a keyframe strip"));
        };
        let Some(k) = act.layers[l].strips[s].keyframe_mut() else {
            return Err(internal("not a keyframe strip"));
        };
        let b = if ensure {
            Some(k.channelbag_ensure(handle))
        } else {
            k.channelbag_index(handle)
        };
        Ok(match b {
            Some(b) => loc_ptr(a, act, Loc::Bag(l, s, b)),
            None => Value::None,
        })
    })
}
