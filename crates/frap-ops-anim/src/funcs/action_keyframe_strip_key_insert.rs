// SPDX-License-Identifier: GPL-3.0-or-later
//! `strip.key_insert(slot, data_path, array_index, value, time) -> bool`
//! (`rna_ActionStrip_key_insert` → `StripKeyframeData::keyframe_insert`, Blender 5.2.4): ensures
//! the slot's channelbag and the F-curve `data_path[array_index]`, then inserts a key with the
//! preferences' settings (BEZIER, AUTO_CLAMPED) and recalculates the handles. A locked F-curve
//! (or one in a locked group) takes no key: `False`. `slot=None` raises
//! `RuntimeError: Error: Slot cannot be None`.

use frap_anim::fcurve::keys::{InsertFlags, InsertResult, KeyframeSettings, insert_vert};
use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{
    arg_f32, arg_int, arg_ptr, arg_str, internal, report_error, slot_arg, with_action,
};

#[rna_func(srna = "ActionKeyframeStrip", func = "key_insert")]
fn action_keyframe_strip_key_insert(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let Some((_, handle)) = arg_ptr(args, "slot").and_then(|p| slot_arg(h.main(), p)) else {
        return Err(report_error("Slot cannot be None"));
    };
    let data_path = arg_str(args, "data_path").to_owned();
    let index = i32::try_from(arg_int(args, "array_index", -1)).unwrap_or(-1);
    let value = arg_f32(args, "value", 0.0);
    let time = arg_f32(args, "time", 0.0);
    with_action(h, this, |act, loc| {
        let Loc::Strip(l, s) = loc else {
            return Err(internal("key_insert() outside a keyframe strip"));
        };
        let Some(k) = act.layers[l].strips[s].keyframe_mut() else {
            return Err(internal("not a keyframe strip"));
        };
        let b = k.channelbag_ensure(handle);
        let bag = &mut k.channelbags[b];
        let f = bag.fcurve_ensure(&data_path, index, None);
        let group_locked = bag.fcurves[f]
            .group
            .as_ref()
            .and_then(|g| bag.groups.iter().find(|x| &x.name == g))
            .is_some_and(|g| g.lock);
        let fc = &mut bag.fcurves[f];
        if fc.lock || group_locked {
            return Ok(Value::Bool(false));
        }
        let r = insert_vert(
            fc,
            [time, value],
            &KeyframeSettings::default(),
            InsertFlags::default(),
        );
        Ok(Value::Bool(matches!(r, InsertResult::Success(_))))
    })
}
