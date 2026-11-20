// SPDX-License-Identifier: GPL-3.0-or-later
//! `PoseBone` and `Pose` hooks (WP 6.2). `Object.pose` is a hook, so every pose write is a set
//! hook here; the pose matrices are computed fresh on every read (`edit_armature::posing`,
//! deviation D-04: Blender reads them as of the last depsgraph evaluation). Ports of
//! blender/source/blender/makesrna/intern/rna_pose.cc (`rna_PoseChannel_rotation_mode_set`,
//! `rna_PoseChannel_matrix_set`, `rna_PoseChannel_matrix_basis_get/set`,
//! `rna_PoseChannel_length_get`, `rna_PoseChannel_bone_get`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! With `Armature.pose_position = 'REST'` the matrices are the rest matrices
//! (`BKE_pose_where_is`). In Edit Mode they stay the pose of the stored (pre-edit) bones and
//! the channels: Blender's pose evaluation skips armatures with edit bones.

use frap_rna::traits::{FromRnaValue, IdField, ToRnaValue};
use frap_rna::{Field, IdRef, MathTag, ObjectId, PropDef, RnaError, RnaPtr, Value, rna_hook};
use glam::Mat4;
use smol_str::SmolStr;

use crate::edit_armature::paths::{
    ItemKey, bone_ptr, internal_error, object_armature, object_of, pose_bone_of, pose_bone_ptr,
};
use crate::edit_armature::posing::{PoseMats, pose_to_bone, where_is};
use crate::edit_armature::rename::rename_bone;
use crate::edit_armature::rest::{RestBone, rest_pose};
use crate::id::{Armature, Object, PoseBone};
use crate::{Host, HostRead};

/// What a pose-bone pointer resolves to.
struct Located<'h> {
    ob: IdRef<ObjectId>,
    object: &'h Object,
    arm: IdRef<frap_rna::ArmatureId>,
    armature: &'h Armature,
    channel: &'h PoseBone,
    /// Index of the channel's bone in the armature (`None`: a channel without a bone).
    bone: Option<usize>,
}

fn located<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<Located<'h>, RnaError> {
    let (ob, key) = pose_bone_of(this)?;
    let object = h
        .main()
        .objects
        .get(ob)
        .ok_or_else(|| internal_error("the object was removed"))?;
    let arm =
        object_armature(h.main(), ob).ok_or_else(|| internal_error("not an armature object"))?;
    let armature = h
        .main()
        .armatures
        .get(arm)
        .ok_or_else(|| internal_error("the armature was removed"))?;
    let pose = object
        .pose
        .as_ref()
        .ok_or_else(|| internal_error("the pose was removed"))?;
    let ci = key
        .find(pose.bones.iter().map(|b| b.name.as_str()))
        .ok_or_else(|| internal_error("the pose bone was removed"))?;
    let channel = &pose.bones[ci];
    let bone = armature.bone_index(&channel.name);
    Ok(Located {
        ob,
        object,
        arm,
        armature,
        channel,
        bone,
    })
}

/// Whether the pose reads the rest bones (`pose_position = 'REST'`). Edit Mode keeps the
/// pose: Blender's pose evaluation returns early while `arm->edbo` is set
/// (armature_update.cc), so the channels keep the matrices of the stored (pre-edit) bones.
fn rest_position(_h: &dyn HostRead, l: &Located) -> bool {
    l.armature.pose_position == "REST"
}

/// Rest data and evaluated pose matrices of the whole armature.
fn evaluate(h: &dyn HostRead, l: &Located) -> (Vec<RestBone>, Vec<PoseMats>) {
    let rest = rest_pose(l.armature);
    let mats = where_is(
        l.armature,
        &rest,
        l.object.pose.as_ref(),
        rest_position(h, l),
    );
    (rest, mats)
}

/// The evaluated matrices of the pointed channel (identity for a channel without a bone).
fn mats_of(h: &dyn HostRead, this: &RnaPtr) -> Result<PoseMats, RnaError> {
    let l = located(h, this)?;
    Ok(match l.bone {
        Some(i) => evaluate(h, &l).1[i],
        None => PoseMats {
            pose_mat: Mat4::IDENTITY,
            head: glam::Vec3::ZERO,
            tail: glam::Vec3::ZERO,
            chan_mat: Mat4::IDENTITY,
        },
    })
}

/// Runs `f` on the pointed channel of the object (logged as modified).
fn with_channel(
    h: &mut dyn Host,
    ob: IdRef<ObjectId>,
    key: &ItemKey,
    f: impl FnOnce(&mut PoseBone) -> Result<(), RnaError>,
) -> Result<(), RnaError> {
    let (main, log) = h.main_mut();
    let o = main
        .objects
        .get_mut(ob, log)
        .ok_or_else(|| internal_error("the object was removed"))?;
    let pose = o
        .pose
        .as_mut()
        .ok_or_else(|| internal_error("the pose was removed"))?;
    let i = key
        .find(pose.bones.iter().map(|b| b.name.as_str()))
        .ok_or_else(|| internal_error("the pose bone was removed"))?;
    f(&mut pose.bones[i])
}

/// The plain-field setter shared by the `(set = hook)` bindings.
fn set_field(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let (ob, key) = pose_bone_of(this)?;
    with_channel(h, ob, &key, |pb| {
        match prop.identifier {
            "location" => pb.location = <[f32; 3]>::from_rna_value(prop, v)?,
            "scale" => pb.scale = <[f32; 3]>::from_rna_value(prop, v)?,
            "rotation_quaternion" => pb.rotation_quaternion = <[f32; 4]>::from_rna_value(prop, v)?,
            "rotation_axis_angle" => pb.rotation_axis_angle = <[f32; 4]>::from_rna_value(prop, v)?,
            "rotation_euler" => pb.rotation_euler = <[f32; 3]>::from_rna_value(prop, v)?,
            "rotation_mode" => {
                let new = SmolStr::from_rna_value(prop, v)?;
                pb.set_rotation_mode(&new);
            }
            "hide" => pb.hide = bool::from_rna_value(prop, v)?,
            "select" => pb.select = bool::from_rna_value(prop, v)?,
            "lock_location" => pb.lock_location = <[bool; 3]>::from_rna_value(prop, v)?,
            "lock_rotation" => pb.lock_rotation = <[bool; 3]>::from_rna_value(prop, v)?,
            "lock_rotation_w" => pb.lock_rotation_w = bool::from_rna_value(prop, v)?,
            "lock_rotations_4d" => pb.lock_rotations_4d = bool::from_rna_value(prop, v)?,
            "lock_scale" => pb.lock_scale = <[bool; 3]>::from_rna_value(prop, v)?,
            _ => {}
        }
        Ok(())
    })
}

macro_rules! field_hooks {
    ($($f:ident = $p:literal),* $(,)?) => {
        $(
            #[rna_hook(srna = "PoseBone", prop = $p, krate = crate)]
            fn $f(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
                set_field(h, this, prop, v)
            }
        )*
    };
}

field_hooks! {
    set_location = "location",
    set_scale = "scale",
    set_rotation_quaternion = "rotation_quaternion",
    set_rotation_axis_angle = "rotation_axis_angle",
    set_rotation_euler = "rotation_euler",
    set_rotation_mode_hook = "rotation_mode",
    set_hide = "hide",
    set_select = "select",
    set_lock_location = "lock_location",
    set_lock_rotation = "lock_rotation",
    set_lock_rotation_w = "lock_rotation_w",
    set_lock_rotations_4d = "lock_rotations_4d",
    set_lock_scale = "lock_scale",
}

#[rna_hook(srna = "PoseBone", prop = "name", krate = crate)]
fn set_name(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let (arm, old) = {
        let l = located(h, this)?;
        (l.arm, l.channel.name.clone())
    };
    let new = SmolStr::from_rna_value(prop, v)?;
    rename_bone(h, arm, &old, &new);
    Ok(())
}

#[rna_hook(srna = "PoseBone", prop = "bone", krate = crate)]
fn bone<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let l = located(h, this)?;
    Ok(match l.bone {
        Some(i) => Field::Ptr(bone_ptr(l.arm, &l.armature.bones[i].name)),
        None => Field::Value(Value::None),
    })
}

#[rna_hook(srna = "PoseBone", prop = "parent", krate = crate)]
fn parent<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let l = located(h, this)?;
    let parent = l
        .bone
        .and_then(|i| l.armature.bones[i].parent)
        .map(|p| l.armature.bones[p].name.clone())
        .filter(|n| l.object.pose.as_ref().is_some_and(|p| p.bone(n).is_some()));
    Ok(match parent {
        Some(n) => Field::Ptr(pose_bone_ptr(l.ob, &n)),
        None => Field::Value(Value::None),
    })
}

/// `PoseBone.child`: the last connected child in bone order (`rebuild_pose_from_armature`).
#[rna_hook(srna = "PoseBone", prop = "child", krate = crate)]
fn child<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let l = located(h, this)?;
    let child = l.bone.and_then(|i| {
        l.armature
            .bones
            .iter()
            .rev()
            .find(|b| b.parent == Some(i) && b.use_connect)
            .map(|b| b.name.clone())
    });
    Ok(match child {
        Some(n) => Field::Ptr(pose_bone_ptr(l.ob, &n)),
        None => Field::Value(Value::None),
    })
}

#[rna_hook(srna = "PoseBone", prop = "head", krate = crate)]
fn head<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let m = mats_of(h, this)?;
    Ok(Field::Value(
        m.head.to_array().to_rna_value(prop, MathTag::Vector),
    ))
}

#[rna_hook(srna = "PoseBone", prop = "tail", krate = crate)]
fn tail<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let m = mats_of(h, this)?;
    Ok(Field::Value(
        m.tail.to_array().to_rna_value(prop, MathTag::Vector),
    ))
}

#[rna_hook(srna = "PoseBone", prop = "length", krate = crate)]
fn length<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let m = mats_of(h, this)?;
    Ok(Field::Value(
        (m.tail - m.head).length().to_rna_value(prop, MathTag::None),
    ))
}

#[rna_hook(srna = "PoseBone", prop = "matrix_channel", krate = crate)]
fn matrix_channel<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let m = mats_of(h, this)?;
    Ok(Field::Value(m.chan_mat.to_rna_value(prop, MathTag::Matrix)))
}

#[rna_hook(srna = "PoseBone", prop = "matrix", krate = crate)]
mod matrix {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let m = mats_of(h, this)?;
        Ok(Field::Value(m.pose_mat.to_rna_value(prop, MathTag::Matrix)))
    }

    /// `rna_PoseChannel_matrix_set`: the local channels that give the bone this armature-space
    /// matrix with the parent's current pose (no Euler compatibility).
    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let m = Mat4::from_rna_value(prop, v)?;
        let (ob, key) = pose_bone_of(this)?;
        let local = {
            let l = located(h, this)?;
            let Some(i) = l.bone else {
                return Ok(());
            };
            let (rest, mats) = evaluate(h, &l);
            let parent_pose = l.armature.bones[i].parent.map(|p| mats[p].pose_mat);
            pose_to_bone(l.armature, &rest, i, parent_pose.as_ref(), &m)
        };
        with_channel(h, ob, &key, |pb| {
            let mut c = pb.channels();
            c.apply_mat4(&local, false);
            pb.set_channels(&c);
            Ok(())
        })
    }
}

#[rna_hook(srna = "PoseBone", prop = "matrix_basis", krate = crate)]
mod matrix_basis {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let l = located(h, this)?;
        let connected = l.bone.is_some_and(|i| l.armature.bones[i].use_connect);
        Ok(Field::Value(
            l.channel
                .channels()
                .to_mat4(connected)
                .to_rna_value(prop, MathTag::Matrix),
        ))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let m = Mat4::from_rna_value(prop, v)?;
        let (ob, key) = pose_bone_of(this)?;
        with_channel(h, ob, &key, |pb| {
            let mut c = pb.channels();
            c.apply_mat4(&m, false);
            pb.set_channels(&c);
            Ok(())
        })
    }
}

#[rna_hook(srna = "PoseBone", prop = "custom_shape", krate = crate)]
mod custom_shape {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let l = located(h, this)?;
        Ok(Field::Id(l.channel.custom_shape.map(|o| o.any())))
    }

    /// `rna_Pose_custom_shape_object_poll`: armature objects are refused (silently, as a failed
    /// pointer poll leaves the value).
    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let target: Option<IdRef<ObjectId>> = IdField::set_id(prop, v)?;
        if let Some(t) = target
            && h.main()
                .objects
                .get(t)
                .is_some_and(|o| o.object_type() == "ARMATURE")
        {
            return Ok(());
        }
        let (ob, key) = pose_bone_of(this)?;
        with_channel(h, ob, &key, |pb| {
            pb.custom_shape = target;
            Ok(())
        })
    }
}

/// The `Pose` flags (`use_mirror_x`, `use_mirror_relative`, `use_auto_ik`).
fn set_pose_flag(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let on = bool::from_rna_value(prop, v)?;
    let (main, log) = h.main_mut();
    let o = main
        .objects
        .get_mut(ob, log)
        .ok_or_else(|| internal_error("the object was removed"))?;
    if let Some(p) = o.pose.as_mut() {
        match prop.identifier {
            "use_mirror_x" => p.use_mirror_x = on,
            "use_mirror_relative" => p.use_mirror_relative = on,
            "use_auto_ik" => p.use_auto_ik = on,
            _ => {}
        }
    }
    Ok(())
}

#[rna_hook(srna = "Pose", prop = "use_mirror_x", krate = crate)]
fn set_use_mirror_x(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_pose_flag(h, this, prop, v)
}

#[rna_hook(srna = "Pose", prop = "use_mirror_relative", krate = crate)]
fn set_use_mirror_relative(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_pose_flag(h, this, prop, v)
}

#[rna_hook(srna = "Pose", prop = "use_auto_ik", krate = crate)]
fn set_use_auto_ik(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_pose_flag(h, this, prop, v)
}
