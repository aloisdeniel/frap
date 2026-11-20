// SPDX-License-Identifier: GPL-3.0-or-later
//! `strip.channelbags.new(slot)` (`rna_Channelbags_new`, Blender 5.2.4): an empty channelbag
//! for the slot, appended. A second one for the same slot raises
//! `RuntimeError: Error: A channelbag for this slot already exists`.

use frap_anim::Channelbag;
use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{
    action_of, arg_ptr, internal, loc_ptr, report_error, slot_arg, with_action,
};

#[rna_func(srna = "ActionChannelbags", func = "new")]
fn action_channelbags_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let Some((_, handle)) = arg_ptr(args, "slot").and_then(|p| slot_arg(h.main(), p)) else {
        return Err(internal("channelbags.new(): no slot"));
    };
    let a = action_of(this)?;
    with_action(h, this, |act, loc| {
        let Loc::Strip(l, s) = loc else {
            return Err(internal("channelbags.new() outside a keyframe strip"));
        };
        let Some(k) = act.layers[l].strips[s].keyframe_mut() else {
            return Err(internal("not a keyframe strip"));
        };
        if k.channelbag_index(handle).is_some() {
            return Err(report_error("A channelbag for this slot already exists"));
        }
        k.channelbags.push(Channelbag {
            slot_handle: handle,
            ..Channelbag::default()
        });
        let b = k.channelbags.len() - 1;
        Ok(loc_ptr(a, act, Loc::Bag(l, s, b)))
    })
}
