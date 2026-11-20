// SPDX-License-Identifier: GPL-3.0-or-later
//! `EditBone.align_roll(vector)` (M5+M6 stabilization): the roll that turns the bone's Z axis
//! towards `vector`. Ported from blender/source/blender/makesrna/intern/rna_armature_api.cc
//! (`rna_EditBone_align_roll`: `ebo->roll = ED_armature_ebone_roll_to_vector(ebo, no, false)`,
//! Blender 5.2.4, GPL-2.0-or-later); the vector is used as given (Blender only asserts it is
//! unit length in debug builds). No transform update runs, as in Blender.

use std::sync::Arc;

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use glam::Vec3;

use crate::edit_armature::paths::{edit_bone_of, internal_error, stale_edit_bone};
use crate::edit_armature::tools::roll_to_vector;
use crate::{FuncArgs, Host};

#[rna_func(srna = "EditBone", func = "align_roll", krate = crate)]
fn edit_bone_align_roll(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let v = match args.get("vector") {
        Some(Value::FloatArray { data, .. }) if data.len() == 3 => {
            Vec3::new(data[0], data[1], data[2])
        }
        _ => return Err(internal_error("align_roll(): vector is not a 3D vector")),
    };
    let (arm, key) = edit_bone_of(this)?;
    let ea = h
        .edit_mut()
        .armatures
        .get_mut(&arm.key())
        .ok_or_else(stale_edit_bone)?;
    let i = key
        .find(ea.edit_bones.iter().map(|b| b.name.as_str()))
        .ok_or_else(stale_edit_bone)?;
    let ea = Arc::make_mut(ea);
    ea.edit_bones[i].roll = roll_to_vector(&ea.edit_bones[i], v, false);
    Ok(Value::None)
}
