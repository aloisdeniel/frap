// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.keyframe_delete(type='DEFAULT')`, tier T0 (WP 6.4). Port of `delete_key_exec`
//! (blender/source/blender/editors/animation/keyframing.cc), Blender 5.2.4, GPL-2.0-or-later:
//! `DEFAULT` means the scene's active keying set (`ANIM_keyingset_get_from_enum_type(scene,
//! 0)`); frap scenes have none (`Scene.keying_sets` are not modelled), so the call raises
//! Blender's `RuntimeError: Error: No active Keying Set`. `type` is a dynamic enum
//! (`ANIM_keying_sets_enum_itemf`, see `anim.keyframe_insert`): a named built-in keying set
//! removes the keys of its channels at the current frame, as `anim.keyframe_delete_by_name`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::anim;

use super::anim__keyframe_insert::common;

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    notes = [
        "type lists the built-in keying sets whose poll passes in the context; unset, there is no active keying set (frap scenes have none) and the call fails as in Blender",
    ],
    dyn_enums = [("type", common::keying_set_items)]
)]
fn exec(cx: &mut OpCtx, p: anim::KeyframeDelete) -> OpResult {
    if p.r#type.value == "DEFAULT" {
        return Err(OpError::Runtime("No active Keying Set".into()));
    }
    let ks = common::find(&p.r#type.value)?;
    common::delete_with_keyingset(cx, ks)
}
