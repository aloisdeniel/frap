// SPDX-License-Identifier: GPL-3.0-or-later
//! Canonical RNA pointers of bones and their parsing (WP 6.2): the hooks of `Bone`,
//! `EditBone`, `PoseBone` and `BoneCollection` find their data from the pointer they are
//! called with, and every bone-valued read answers Blender's canonical pointer
//! (`bpy.data.armatures['Armature'].bones["Bone"]`,
//! `bpy.data.objects['Armature'].pose.bones["Bone"]`, `….edit_bones["Bone"]`).

use frap_rna::{ArmatureId, IdRef, ObjectId, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, Seg};
use smol_str::SmolStr;

use crate::data::Main;
use crate::id::Armature;

/// An internal error (a pointer this hook cannot serve).
pub fn internal_error(msg: impl Into<String>) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("frap internal: {}", msg.into()),
    )
}

/// The item a collection segment names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemKey {
    /// `["Name"]`
    Name(SmolStr),
    /// `[i]`
    Index(usize),
}

impl ItemKey {
    /// The index of the item among `names`.
    pub fn find<'a>(&self, names: impl Iterator<Item = &'a str>) -> Option<usize> {
        let mut names = names;
        match self {
            ItemKey::Name(n) => names.position(|x| x == n.as_str()),
            ItemKey::Index(i) => names.nth(*i).map(|_| *i),
        }
    }
}

fn key_after(this: &RnaPtr, prefix: &[&str]) -> Result<ItemKey, RnaError> {
    let n = prefix.len();
    let ok = this.path.len() > n
        && prefix
            .iter()
            .zip(&this.path)
            .all(|(p, s)| matches!(s, Seg::Prop(x) if x == p));
    if !ok {
        return Err(internal_error(format!(
            "{} pointer is not below {}",
            this.srna.identifier,
            prefix.join(".")
        )));
    }
    match &this.path[n] {
        Seg::Key(k) => Ok(ItemKey::Name(k.clone())),
        Seg::Index(i) => usize::try_from(*i)
            .map(ItemKey::Index)
            .map_err(|_| internal_error("negative bone index")),
        Seg::Prop(_) => Err(internal_error("bone pointer without a key")),
    }
}

/// The armature an `Armature`-rooted pointer belongs to.
pub fn armature_of(this: &RnaPtr) -> Result<IdRef<ArmatureId>, RnaError> {
    match this.root {
        Root::Id(id) => id
            .typed::<ArmatureId>()
            .ok_or_else(|| internal_error("pointer is not rooted at an armature")),
        _ => Err(internal_error("pointer is not rooted at an armature")),
    }
}

/// The object an `Object`-rooted pointer belongs to.
pub fn object_of(this: &RnaPtr) -> Result<IdRef<ObjectId>, RnaError> {
    match this.root {
        Root::Id(id) => id
            .typed::<ObjectId>()
            .ok_or_else(|| internal_error("pointer is not rooted at an object")),
        _ => Err(internal_error("pointer is not rooted at an object")),
    }
}

/// `(armature, bone)` of a `Bone` pointer (`….bones[key]`).
pub fn bone_of(this: &RnaPtr) -> Result<(IdRef<ArmatureId>, ItemKey), RnaError> {
    Ok((armature_of(this)?, key_after(this, &["bones"])?))
}

/// `(armature, edit bone)` of an `EditBone` pointer (`….edit_bones[key]`).
pub fn edit_bone_of(this: &RnaPtr) -> Result<(IdRef<ArmatureId>, ItemKey), RnaError> {
    Ok((armature_of(this)?, key_after(this, &["edit_bones"])?))
}

/// `(object, channel)` of a `PoseBone` pointer (`….pose.bones[key]`).
pub fn pose_bone_of(this: &RnaPtr) -> Result<(IdRef<ObjectId>, ItemKey), RnaError> {
    Ok((object_of(this)?, key_after(this, &["pose", "bones"])?))
}

/// `(armature, collection)` of a `BoneCollection` pointer (`….collections_all[key]` or
/// `….collections[key]`).
pub fn collection_of(this: &RnaPtr) -> Result<(IdRef<ArmatureId>, ItemKey), RnaError> {
    let arm = armature_of(this)?;
    key_after(this, &["collections_all"])
        .or_else(|_| key_after(this, &["collections"]))
        .map(|k| (arm, k))
}

/// `bpy.data.armatures[…].bones["name"]`.
pub fn bone_ptr(arm: IdRef<ArmatureId>, name: &str) -> RnaPtr {
    RnaPtr::id(arm.any(), &frap_spec_rna::ARMATURE)
        .child(Seg::Prop("bones"), &frap_spec_rna::BONE)
        .child(Seg::Key(name.into()), &frap_spec_rna::BONE)
}

/// `bpy.data.armatures[…].edit_bones["name"]`.
pub fn edit_bone_ptr(arm: IdRef<ArmatureId>, name: &str) -> RnaPtr {
    RnaPtr::id(arm.any(), &frap_spec_rna::ARMATURE)
        .child(Seg::Prop("edit_bones"), &frap_spec_rna::EDIT_BONE)
        .child(Seg::Key(name.into()), &frap_spec_rna::EDIT_BONE)
}

/// `bpy.data.objects[…].pose.bones["name"]`.
pub fn pose_bone_ptr(ob: IdRef<ObjectId>, name: &str) -> RnaPtr {
    RnaPtr::id(ob.any(), &frap_spec_rna::OBJECT)
        .child(Seg::Prop("pose"), &frap_spec_rna::POSE)
        .child(Seg::Prop("bones"), &frap_spec_rna::POSE_BONE)
        .child(Seg::Key(name.into()), &frap_spec_rna::POSE_BONE)
}

/// `bpy.data.armatures[…].collections_all["name"]`.
pub fn collection_ptr(arm: IdRef<ArmatureId>, name: &str) -> RnaPtr {
    RnaPtr::id(arm.any(), &frap_spec_rna::ARMATURE)
        .child(
            Seg::Prop("collections_all"),
            &frap_spec_rna::BONE_COLLECTION,
        )
        .child(Seg::Key(name.into()), &frap_spec_rna::BONE_COLLECTION)
}

/// The armature data of object `ob`, if it is an armature object.
pub fn object_armature(main: &Main, ob: IdRef<ObjectId>) -> Option<IdRef<ArmatureId>> {
    main.objects.get(ob)?.data?.typed::<ArmatureId>()
}

/// The armature `arm`, or an error naming the removed datablock.
pub fn armature<'m>(main: &'m Main, arm: IdRef<ArmatureId>) -> Result<&'m Armature, RnaError> {
    main.armatures
        .get(arm)
        .ok_or_else(|| internal_error("the armature was removed"))
}

/// Blender's message for an edit-bone pointer whose armature left Edit Mode (the `EditBone`
/// was freed; Blender raises `ReferenceError`, which frap's exception set maps to
/// `RuntimeError`).
pub fn stale_edit_bone() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "StructRNA of type EditBone has been removed",
    )
}
