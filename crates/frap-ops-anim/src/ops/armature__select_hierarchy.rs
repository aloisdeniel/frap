// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.select_hierarchy(direction='PARENT', extend=False)`, tier T0
//! (WP 8.3.10). Port of `armature_select_hierarchy_exec`
//! (blender/source/blender/editors/armature/armature_select.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::select_hierarchy`).
//!
//! On the edit object's armature only: the active bone's parent (`PARENT`) or first selectable
//! child (`CHILD`, a connected child first) becomes active and selected; the old active bone is
//! deselected unless `extend`. `CANCELLED` without an active bone or a target.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature::{self, select_hierarchy::Direction};

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SELECT_HIERARCHY
)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureSelectHierarchy) -> OpResult {
    let Some((_, arm)) = common::edit_armatures(cx).into_iter().next() else {
        return Ok(OpStatus::Cancelled);
    };
    let parent = p.direction.value == Direction::Parent;
    let extend = p.extend.value;
    let changed = common::with_edit(cx, arm, |ea, a| ea.select_hierarchy(a, parent, extend));
    Ok(if changed == Some(true) {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
