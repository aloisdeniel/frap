// SPDX-License-Identifier: GPL-3.0-or-later
//! `EditBone` hooks (WP 6.2). Edit bones live in `EditState`, so every write is a set hook,
//! with Blender's update callbacks (blender/source/blender/makesrna/intern/rna_armature.cc,
//! Blender 5.2.4, GPL-2.0-or-later):
//!
//! - `head`, `tail`, `roll`, `length`, `head_radius`, `tail_radius`, `envelope_distance`,
//!   `bbone_segments` run `rna_Armature_editbone_transform_update`: a connected parent's tail
//!   and connected children's heads (and radii) follow; with `use_mirror_x` the X mirror bone
//!   is updated too (`ED_armature_ebone_transform_mirror_update`);
//! - `use_connect` snaps the head to the parent's tail (`rna_EditBone_connected_check`);
//! - `parent` refuses itself and its descendants silently, and a bone of another armature;
//! - `hide` / `hide_select` deselect the bone (`rna_EditBone_hide_update`);
//! - `matrix` sets head, direction and roll keeping the length (`ED_armature_ebone_from_mat4`);
//! - `name` renames through `ED_armature_bone_rename`.

use std::sync::Arc;

use smol_str::SmolStr;
use frap_rna::traits::{FromRnaValue, ToRnaValue};
use frap_rna::{ArmatureId, Field, IdRef, MathTag, PropDef, RnaError, RnaPtr, Value, rna_hook};
use glam::{Mat4, Vec3};

use crate::edit_armature::paths::{
    armature, edit_bone_of, edit_bone_ptr, ref_coll, stale_edit_bone,
};
use crate::edit_armature::rename::rename_bone;
use crate::edit_armature::{EditArmature, ROOT, SEL, TIP};
use crate::{Host, HostRead};

/// The edit armature and the bone index a pointer names.
fn located<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<(IdRef<ArmatureId>, &'h EditArmature, usize), RnaError> {
    let (arm, key) = edit_bone_of(this)?;
    let ea = h
        .edit()
        .armatures
        .get(&arm.key())
        .ok_or_else(stale_edit_bone)?;
    let i = key
        .find(ea.edit_bones.iter().map(|b| b.name.as_str()))
        .ok_or_else(stale_edit_bone)?;
    Ok((arm, ea, i))
}

/// Runs `f` on the edit armature (copy-on-write) and bone index a pointer names, then the
/// X-mirror update of bone `i` when `mirror` and the armature has `use_mirror_x`.
fn with_bone(
    h: &mut dyn Host,
    this: &RnaPtr,
    mirror: bool,
    f: impl FnOnce(&mut EditArmature, usize) -> Result<(), RnaError>,
) -> Result<(), RnaError> {
    let (arm, _, i) = located(h, this)?;
    let a = armature(h.main(), arm)?.clone();
    let ea = h
        .edit_mut()
        .armatures
        .get_mut(&arm.key())
        .ok_or_else(stale_edit_bone)?;
    let ea = Arc::make_mut(ea);
    f(ea, i)?;
    if mirror && a.use_mirror_x {
        ea.transform_mirror_update(&a, i, false);
    }
    Ok(())
}

/// The plain-field setter shared by the `(set = hook)` bindings.
fn set_field(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let id = prop.identifier;
    let transform = matches!(
        id,
        "head" | "tail" | "roll" | "head_radius" | "tail_radius" | "envelope_distance" | "bbone_segments"
    );
    with_bone(h, this, transform, |ea, i| {
        let b = &mut ea.edit_bones[i];
        match id {
            "head" => b.head = <[f32; 3]>::from_rna_value(prop, v)?,
            "tail" => b.tail = <[f32; 3]>::from_rna_value(prop, v)?,
            "roll" => b.roll = f32::from_rna_value(prop, v)?,
            "head_radius" => b.head_radius = f32::from_rna_value(prop, v)?,
            "tail_radius" => b.tail_radius = f32::from_rna_value(prop, v)?,
            "envelope_distance" => b.envelope_distance = f32::from_rna_value(prop, v)?,
            "envelope_weight" => b.envelope_weight = f32::from_rna_value(prop, v)?,
            "bbone_segments" => b.bbone_segments = i32::from_rna_value(prop, v)?,
            "use_inherit_rotation" => b.use_inherit_rotation = bool::from_rna_value(prop, v)?,
            "use_envelope_multiply" => b.use_envelope_multiply = bool::from_rna_value(prop, v)?,
            "use_deform" => b.use_deform = bool::from_rna_value(prop, v)?,
            "use_local_location" => b.use_local_location = bool::from_rna_value(prop, v)?,
            "use_relative_parent" => b.use_relative_parent = bool::from_rna_value(prop, v)?,
            "inherit_scale" => b.inherit_scale = SmolStr::from_rna_value(prop, v)?,
            "lock" => b.lock = bool::from_rna_value(prop, v)?,
            "hide" | "hide_select" => {
                let on = bool::from_rna_value(prop, v)?;
                if id == "hide" {
                    b.hide = on;
                } else {
                    b.hide_select = on;
                }
                if b.hide || b.hide_select {
                    b.select = [false; 3];
                }
            }
            "use_connect" => {
                b.use_connect = bool::from_rna_value(prop, v)?;
                connected_check(ea, i);
            }
            _ => {}
        }
        if transform {
            ea.transform_update(i);
        }
        Ok(())
    })
}

/// `rna_EditBone_connected_check`.
fn connected_check(ea: &mut EditArmature, i: usize) {
    let Some(p) = ea.edit_bones[i].parent else {
        return;
    };
    if ea.edit_bones[i].use_connect {
        ea.edit_bones[i].head = ea.edit_bones[p].tail;
        if ea.edit_bones[i].select[ROOT] {
            ea.edit_bones[p].select[TIP] = true;
        }
    } else if !ea.edit_bones[p].select[ROOT] {
        ea.edit_bones[p].select[TIP] = false;
    }
}

macro_rules! field_hooks {
    ($($f:ident = $p:literal),* $(,)?) => {
        $(
            #[rna_hook(srna = "EditBone", prop = $p, krate = crate)]
            fn $f(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
                set_field(h, this, prop, v)
            }
        )*
    };
}

field_hooks! {
    set_head = "head",
    set_tail = "tail",
    set_roll = "roll",
    set_head_radius = "head_radius",
    set_tail_radius = "tail_radius",
    set_envelope_distance = "envelope_distance",
    set_envelope_weight = "envelope_weight",
    set_bbone_segments = "bbone_segments",
    set_use_inherit_rotation = "use_inherit_rotation",
    set_use_envelope_multiply = "use_envelope_multiply",
    set_use_deform = "use_deform",
    set_use_local_location = "use_local_location",
    set_use_relative_parent = "use_relative_parent",
    set_inherit_scale = "inherit_scale",
    set_lock = "lock",
    set_hide = "hide",
    set_hide_select = "hide_select",
    set_use_connect = "use_connect",
}

#[rna_hook(srna = "EditBone", prop = "name", krate = crate)]
fn set_name(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (arm, ea, i) = located(h, this)?;
    let old = ea.edit_bones[i].name.clone();
    let new = SmolStr::from_rna_value(prop, v)?;
    rename_bone(h, arm, &old, &new);
    Ok(())
}

#[rna_hook(srna = "EditBone", prop = "parent", krate = crate)]
mod parent {
    use super::*;

    pub fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, _prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
        let (arm, ea, i) = located(h, this)?;
        Ok(match ea.edit_bones[i].parent.and_then(|p| ea.edit_bones.get(p)) {
            Some(p) => Field::Ptr(edit_bone_ptr(arm, &p.name)),
            None => Field::Value(Value::None),
        })
    }

    /// `rna_EditBone_parent_set`.
    pub fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
        let (arm, _, _) = located(h, this)?;
        let target = match &v {
            Value::Ptr(p) => match edit_bone_of(p) {
                Ok((a, key)) if a == arm => Some(key),
                _ => return Ok(()),
            },
            _ => None,
        };
        with_bone(h, this, false, |ea, i| {
            match target {
                None => {
                    if let Some(p) = ea.edit_bones[i].parent
                        && !ea.edit_bones[p].select[ROOT]
                    {
                        ea.edit_bones[p].select[TIP] = false;
                    }
                    ea.edit_bones[i].parent = None;
                    ea.edit_bones[i].use_connect = false;
                }
                Some(key) => {
                    let Some(p) = key.find(ea.edit_bones.iter().map(|b| b.name.as_str())) else {
                        return Ok(());
                    };
                    if p == i || ea.is_child_recursive(i, p) {
                        return Ok(());
                    }
                    ea.edit_bones[i].parent = Some(p);
                    connected_check(ea, i);
                }
            }
            Ok(())
        })
    }
}

#[rna_hook(srna = "EditBone", prop = "length", krate = crate)]
mod length {
    use super::*;

    pub fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
        let (_, ea, i) = located(h, this)?;
        Ok(Field::Value(ea.edit_bones[i].length().to_rna_value(prop, MathTag::None)))
    }

    /// `rna_EditBone_length_set`: a zero-length bone grows along +Z.
    pub fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
        let length = f32::from_rna_value(prop, v)?;
        with_bone(h, this, true, |ea, i| {
            let b = &mut ea.edit_bones[i];
            let (mut delta, len) = crate::edit_armature::math::normalize(b.tail_v() - b.head_v());
            if len == 0.0 {
                delta.z = 1.0;
            }
            b.tail = (b.head_v() + delta * length).to_array();
            ea.transform_update(i);
            Ok(())
        })
    }
}

#[rna_hook(srna = "EditBone", prop = "matrix", krate = crate)]
mod matrix {
    use super::*;

    pub fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
        let (_, ea, i) = located(h, this)?;
        Ok(Field::Value(ea.bone_mat4(i).to_rna_value(prop, MathTag::Matrix)))
    }

    pub fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
        let m = Mat4::from_rna_value(prop, v)?;
        with_bone(h, this, false, |ea, i| {
            ea.set_bone_mat4(i, &m);
            Ok(())
        })
    }
}

/// The selection flag a `select*` property maps to.
fn flag_index(prop: &PropDef) -> usize {
    match prop.identifier {
        "select_head" => ROOT,
        "select_tail" => TIP,
        _ => SEL,
    }
}

fn get_flag<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let (_, ea, i) = located(h, this)?;
    Ok(Field::Value(Value::Bool(ea.edit_bones[i].select[flag_index(prop)])))
}

fn set_flag(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let on = bool::from_rna_value(prop, v)?;
    let k = flag_index(prop);
    with_bone(h, this, false, |ea, i| {
        ea.edit_bones[i].select[k] = on;
        Ok(())
    })
}

#[rna_hook(srna = "EditBone", prop = "select", krate = crate)]
mod select {
    use super::*;
    pub fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
        get_flag(h, this, prop)
    }
    pub fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
        set_flag(h, this, prop, v)
    }
}

#[rna_hook(srna = "EditBone", prop = "select_head", krate = crate)]
mod select_head {
    use super::*;
    pub fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
        get_flag(h, this, prop)
    }
    pub fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
        set_flag(h, this, prop, v)
    }
}

#[rna_hook(srna = "EditBone", prop = "select_tail", krate = crate)]
mod select_tail {
    use super::*;
    pub fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
        get_flag(h, this, prop)
    }
    pub fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
        set_flag(h, this, prop, v)
    }
}

#[rna_hook(srna = "EditBone", prop = "collections", krate = crate)]
fn collections<'h>(h: &'h dyn HostRead, this: &RnaPtr, _prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let (arm, ea, i) = located(h, this)?;
    let a = armature(h.main(), arm)?;
    Ok(ref_coll(
        ea.edit_bones[i]
            .collections
            .iter()
            .filter_map(|c| a.collections.iter().find(|x| x.name == *c))
            .collect(),
        &frap_spec_rna::BONE_COLLECTION,
    ))
}

#[allow(dead_code)]
fn _vec(v: Vec3) -> [f32; 3] {
    v.to_array()
}
