// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.keyframe_delete_by_name(type='')`, tier T0 (WP 6.4). Port of
//! `delete_key_exec` / `delete_key_using_keying_set` (blender/source/blender/editors/
//! animation/keyframing.cc) with `apply_keyingset(…, DELETE_KEY)`, Blender 5.2.4,
//! GPL-2.0-or-later: removes the keys at the current frame from the channels of the named
//! built-in keying set (see `anim.keyframe_insert_by_name`); F-curves left empty are removed.
//! Unknown name: `RuntimeError` "Keying set '…' not found".

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::anim;

use super::anim__keyframe_insert::common;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: anim::KeyframeDeleteByName) -> OpResult {
    let ks = common::find(&p.r#type.value)?;
    common::delete_with_keyingset(cx, ks)
}
