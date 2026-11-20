// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.parent_set(type='CONNECTED')`, tier T0 (WP 6.2). Port of
//! `armature_parent_set_exec`, `bone_connect_to_existing_parent` and
//! `bone_connect_to_new_parent` (blender/source/blender/editors/armature/armature_relations.cc),
//! Blender 5.2.4, GPL-2.0-or-later (`EditArmature::parent_set`), on the edit object's armature.
//!
//! The selected, unlocked bones become children of the active bone (`CONNECTED` moves each
//! with its children so its head is at the active bone's tail; `OFFSET` keeps them in place).
//! A loop through the active bone's ancestors is cut. When only the active bone is selected
//! and it has a parent, it is connected to that parent. Without an active bone:
//! `Operation requires an active bone` (`CANCELLED`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature::{self, parent_set::Type};

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::ParentSet) -> OpResult {
    // `CTX_data_edit_object`: the active object only.
    let Some((_, arm)) = common::edit_armatures(cx).into_iter().next() else {
        return Ok(OpStatus::Cancelled);
    };
    let r = common::with_edit(cx, arm, |ea, a| {
        ea.parent_set(a, p.r#type.value == Type::Connected)
    });
    match r {
        Some(Ok(())) => Ok(OpStatus::Finished),
        Some(Err(msg)) => {
            cx.report(ReportType::Error, msg);
            Ok(OpStatus::Cancelled)
        }
        None => Ok(OpStatus::Cancelled),
    }
}
