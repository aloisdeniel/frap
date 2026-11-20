// SPDX-License-Identifier: GPL-3.0-or-later
//! `armature.edit_bones.remove(bone)` (WP 6.2, `rna_Armature_edit_bone_remove` →
//! `ED_armature_ebone_remove`): the bone's children move to its parent and are disconnected;
//! an active bone is no longer active. Blender's errors (a `RuntimeError` each): `Armature '…'
//! not in edit mode, cannot remove an editbone`, `Armature '…' does not contain bone '…'`.

use std::sync::Arc;

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::edit_armature::paths::{armature_of, edit_bone_of};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, report_error};

#[rna_func(srna = "ArmatureEditBones", func = "remove")]
fn armature_edit_bones_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let arm = armature_of(this)?;
    let arm_name = h
        .main()
        .armatures
        .get(arm)
        .map(|a| a.header.name.clone())
        .ok_or_else(|| internal("edit_bones.remove(): the armature was removed"))?;
    let target = arg_ptr(args, "bone")
        .ok_or_else(|| internal("edit_bones.remove(): no bone argument"))?
        .clone();
    let (bone_arm, key) = edit_bone_of(&target)?;
    let Some(ea) = h.edit_mut().armatures.get_mut(&arm.key()) else {
        return Err(report_error(format!(
            "Armature '{arm_name}' not in edit mode, cannot remove an editbone"
        )));
    };
    let ea = Arc::make_mut(ea);
    let found = (bone_arm == arm)
        .then(|| key.find(ea.edit_bones.iter().map(|b| b.name.as_str())))
        .flatten();
    let Some(i) = found else {
        let name = match key {
            frap_scene::edit_armature::paths::ItemKey::Name(n) => n.to_string(),
            frap_scene::edit_armature::paths::ItemKey::Index(i) => i.to_string(),
        };
        return Err(report_error(format!(
            "Armature '{arm_name}' does not contain bone '{name}'"
        )));
    };
    ea.remove_bone(i, true);
    Ok(Value::None)
}
