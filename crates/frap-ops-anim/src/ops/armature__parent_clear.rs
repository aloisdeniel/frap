// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.parent_clear(type='CLEAR')`, tier T0 (WP 6.2). Port of
//! `armature_parent_clear_exec` and `editbone_clear_parent`
//! (blender/source/blender/editors/armature/armature_relations.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! For each of `context.selected_editable_bones`: the parent's tip is deselected, `CLEAR`
//! removes the parent, both types clear the connection; then the selection is re-synced in
//! every armature with an editable bone. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature::{self, parent_clear::Type};

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::ParentClear) -> OpResult {
    let clear = p.r#type.value == Type::Clear;
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| {
            let bones = ea.selected_bones(a, true);
            ea.parent_clear(&bones, clear);
            if ea.any_editable() {
                ea.sync_selection();
            }
        });
    }
    Ok(OpStatus::Finished)
}
