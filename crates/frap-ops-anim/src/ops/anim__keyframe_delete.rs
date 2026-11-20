// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.keyframe_delete(type='DEFAULT')`, tier T0 (WP 6.4). Port of `delete_key_exec`
//! (blender/source/blender/editors/animation/keyframing.cc), Blender 5.2.4, GPL-2.0-or-later:
//! `DEFAULT` means the scene's active keying set (`ANIM_keyingset_get_from_enum_type(scene,
//! 0)`); frap scenes have none (`Scene.keying_sets` are not modelled), so the call raises
//! Blender's `RuntimeError: Error: No active Keying Set`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::anim;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(_cx: &mut OpCtx, _p: anim::KeyframeDelete) -> OpResult {
    Err(OpError::Runtime("No active Keying Set".into()))
}
