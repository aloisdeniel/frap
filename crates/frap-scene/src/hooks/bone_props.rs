// SPDX-License-Identifier: GPL-3.0-or-later
//! `Bone` hooks (WP 6.2): the values Blender derives from the whole armature in
//! `armature_finalize_restpose` (`head`/`tail` in the parent's space, `matrix` = `bone_mat`,
//! `length`), the hierarchy (`parent`, `children` in `childbase` order), `collections`, and the
//! `name` setter (`ED_armature_bone_rename`). Values come from `edit_armature::rest`, computed
//! from the stored armature-space head, tail and roll.

use smol_str::SmolStr;
use frap_rna::traits::{FromRnaValue, ToRnaValue};
use frap_rna::{Field, MathTag, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::edit_armature::paths::{bone_of, bone_ptr, collection_ptr, internal_error, armature};
use crate::edit_armature::rename::rename_bone;
use crate::edit_armature::rest::{RestBone, rest_pose};
use crate::id::Armature;
use crate::{Host, HostRead};

fn located<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<(&'h Armature, usize), RnaError> {
    let (arm, key) = bone_of(this)?;
    let a = armature(h.main(), arm)?;
    let i = key
        .find(a.bones.iter().map(|b| b.name.as_str()))
        .ok_or_else(|| internal_error("the bone was removed"))?;
    Ok((a, i))
}

fn rest_of(h: &dyn HostRead, this: &RnaPtr) -> Result<RestBone, RnaError> {
    let (a, i) = located(h, this)?;
    Ok(rest_pose(a)[i])
}

#[rna_hook(srna = "Bone", prop = "head", krate = crate)]
fn head<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let r = rest_of(h, this)?;
    Ok(Field::Value(r.head.to_array().to_rna_value(prop, MathTag::Vector)))
}

#[rna_hook(srna = "Bone", prop = "tail", krate = crate)]
fn tail<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let r = rest_of(h, this)?;
    Ok(Field::Value(r.tail.to_array().to_rna_value(prop, MathTag::Vector)))
}

#[rna_hook(srna = "Bone", prop = "matrix", krate = crate)]
fn matrix<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let r = rest_of(h, this)?;
    Ok(Field::Value(r.bone_mat.to_rna_value(prop, MathTag::Matrix)))
}

#[rna_hook(srna = "Bone", prop = "length", krate = crate)]
fn length<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let r = rest_of(h, this)?;
    Ok(Field::Value(r.length.to_rna_value(prop, MathTag::None)))
}

#[rna_hook(srna = "Bone", prop = "parent", krate = crate)]
fn parent<'h>(h: &'h dyn HostRead, this: &RnaPtr, _prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let (arm, _) = bone_of(this)?;
    let (a, i) = located(h, this)?;
    Ok(match a.bones[i].parent.and_then(|p| a.bones.get(p)) {
        Some(p) => Field::Ptr(bone_ptr(arm, &p.name)),
        None => Field::Value(Value::None),
    })
}

#[rna_hook(srna = "Bone", prop = "children", krate = crate)]
fn children<'h>(h: &'h dyn HostRead, this: &RnaPtr, _prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let (arm, _) = bone_of(this)?;
    let (a, i) = located(h, this)?;
    Ok(Field::Value(Value::Tuple(
        a.children_of(Some(i))
            .into_iter()
            .map(|c| Value::Ptr(Box::new(bone_ptr(arm, &a.bones[c].name))))
            .collect(),
    )))
}

#[rna_hook(srna = "Bone", prop = "collections", krate = crate)]
fn collections<'h>(h: &'h dyn HostRead, this: &RnaPtr, _prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let (arm, _) = bone_of(this)?;
    let (a, i) = located(h, this)?;
    Ok(Field::Value(Value::Tuple(
        a.bones[i]
            .collections
            .iter()
            .map(|c| Value::Ptr(Box::new(collection_ptr(arm, c))))
            .collect(),
    )))
}

#[rna_hook(srna = "Bone", prop = "name", krate = crate)]
fn set_name(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (arm, _) = bone_of(this)?;
    let old = {
        let (a, i) = located(h, this)?;
        a.bones[i].name.clone()
    };
    let new = SmolStr::from_rna_value(prop, v)?;
    rename_bone(h, arm, &old, &new);
    Ok(())
}
