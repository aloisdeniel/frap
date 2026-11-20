// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.keyframe_insert_by_name(type='')`, tier T0 (WP 6.4). Port of
//! `keyframe_insert_with_keyingset_exec` (blender/source/blender/editors/animation/
//! keyframing.cc) with `apply_keyingset` (animrig/intern/keyingsets.cc) and the built-in
//! keying sets of `keyingsets_builtins.py`, Blender 5.2.4, GPL-2.0-or-later: `type` names a
//! keying set (`Location`, `Rotation`, `Scaling`, `LocRotScale`, `BUILTIN_KSI_LocRot`, the
//! `Visual…` and `Delta…` sets, `Available`); its channels are keyed on the selected objects,
//! or on the selected pose bones of a posed active object, grouped by object transforms or
//! bone name. Unknown name: `RuntimeError` "Keying set '…' not found"; a context the keying
//! set does not poll for: "No suitable context info for active keying set".
//! `LocRotScaleCProp` keys no custom properties (deviations §2); `BUILTIN_KSI_BendyBones` and
//! the whole-character sets are not implemented.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::anim;

use super::anim__keyframe_insert::common;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: anim::KeyframeInsertByName) -> OpResult {
    let ks = common::find(&p.r#type.value)?;
    common::insert_with_keyingset(cx, ks)
}
