// SPDX-License-Identifier: GPL-3.0-or-later
//! `armature.edit_bones.new(name)` (WP 6.2, `rna_Armature_edit_bone_new` →
//! `ED_armature_ebone_add`): appends an edit bone with a unique name (`''` gives `Bone`), head
//! and tail at the origin (a zero-length bone is elongated when Edit Mode is left), only its
//! tail selected, deforming, envelope 0.25, radii 0.1/0.05. Outside Edit Mode Blender reports
//! `Armature '…' not in edit mode, cannot add an editbone` (a `RuntimeError`).

use std::sync::Arc;

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::edit_armature::paths::{armature_of, edit_bone_ptr};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, internal, report_error};

#[rna_func(srna = "ArmatureEditBones", func = "new")]
fn armature_edit_bones_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new")?;
    let arm = armature_of(this)?;
    let arm_name = h
        .main()
        .armatures
        .get(arm)
        .map(|a| a.header.name.clone())
        .ok_or_else(|| internal("edit_bones.new(): the armature was removed"))?;
    let name = arg_str(args, "name").to_owned();
    let Some(ea) = h.edit_mut().armatures.get_mut(&arm.key()) else {
        return Err(report_error(format!(
            "Armature '{arm_name}' not in edit mode, cannot add an editbone"
        )));
    };
    let ea = Arc::make_mut(ea);
    let i = ea.add_bone(&name);
    Ok(Value::Ptr(Box::new(edit_bone_ptr(
        arm,
        &ea.edit_bones[i].name,
    ))))
}
