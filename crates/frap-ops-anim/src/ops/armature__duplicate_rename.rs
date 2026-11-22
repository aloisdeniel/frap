// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.duplicate_rename(do_flip_names=False, search='', replace='')`, tier T0
//! (WP 8.3.10). Port of `armature_duplicate_rename_exec` and `armature_duplicate_selected`
//! (blender/source/blender/editors/armature/armature_add.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::duplicate_selected_ex`).
//!
//! `armature.duplicate`, with every occurrence of `search` in each copy's name (after the
//! optional side flip) replaced by `replace` before the name is made unique; an empty `search`
//! is an ERROR `No search term defined`. The pose channel of a copy takes the source
//! channel's locks, rotation mode and custom shape. `CANCELLED` without a selected bone.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_DUPLICATE_RENAME
)]
fn exec(cx: &mut OpCtx, p: armature::DuplicateRename) -> OpResult {
    if p.search.value.is_empty() {
        return Err(OpError::Runtime("No search term defined".into()));
    }
    if common::selected_count(cx) == 0 {
        return Ok(OpStatus::Cancelled);
    }
    for (ob, arm) in common::edit_armatures(cx) {
        let Some((a, mut ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        let pairs =
            ea.duplicate_selected_ex(&a, p.do_flip_names.value, &p.search.value, &p.replace.value);
        let names: Vec<(String, String)> = pairs
            .iter()
            .map(|&(s, d)| {
                (
                    ea.edit_bones[s].name.to_string(),
                    ea.edit_bones[d].name.to_string(),
                )
            })
            .collect();
        common::store(cx, arm, ea);
        for (s, d) in names {
            common::copy_pchan(cx, ob, &s, &d)?;
        }
    }
    Ok(OpStatus::Finished)
}
