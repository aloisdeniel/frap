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

/// The item a bone (or bone collection) pointer names. The canonical pointer is
/// `<prefix>[key]`; a pointer reached through a nested collection
/// (`bones["A"].children["B"]`, `collections_all["C"].bones["B"]`) names its item by its last
/// key (bone and collection names are unique within an armature).
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
    match this.path.last() {
        Some(Seg::Key(k)) => Ok(ItemKey::Name(k.clone())),
        Some(Seg::Index(i)) => usize::try_from(*i)
            .map(ItemKey::Index)
            .map_err(|_| internal_error("negative bone index")),
        _ => Err(internal_error("bone pointer without a key")),
    }
}

/// A read-only collection of borrowed structs (`Bone.children`, `BoneCollection.bones`, …).
/// Items are reached through their name (`item_seg` is the default `Key(name)`), which the
/// bone hooks resolve whatever the path above it.
pub struct RefColl<'h, T: frap_rna::Rna> {
    /// The items.
    pub items: Vec<&'h T>,
    /// Their struct type.
    pub item_type: &'static frap_rna::StructDef,
}

impl<T: frap_rna::Rna> frap_rna::RnaColl for RefColl<'_, T> {
    fn item_type(&self) -> &'static frap_rna::StructDef {
        self.item_type
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn key_of(&self, i: usize) -> Option<smol_str::SmolStr> {
        let item = *self.items.get(i)?;
        let prop = item.srna().find_prop(item.srna().name_property?)?;
        match item.get(prop).ok()? {
            frap_rna::Field::Value(frap_rna::Value::Str(s)) => Some(s),
            _ => None,
        }
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.items.len()).find(|i| self.key_of(*i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<frap_rna::Item<'_>, RnaError> {
        self.items
            .get(i)
            .map(|t| frap_rna::Item::Struct(frap_rna::StructRef::Borrowed(*t)))
            .ok_or_else(|| frap_rna::traits::index_out_of_range(i, self.items.len()))
    }
}

/// A [`RefColl`] as a field.
pub fn ref_coll<'h, T: frap_rna::Rna>(
    items: Vec<&'h T>,
    item_type: &'static frap_rna::StructDef,
) -> frap_rna::Field<'h> {
    frap_rna::Field::Coll(frap_rna::CollRef::Owned(Box::new(RefColl {
        items,
        item_type,
    })))
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
    Ok((armature_of(this)?, key_after(this, &[])?))
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
    Ok((armature_of(this)?, key_after(this, &[])?))
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
pub fn armature(main: &Main, arm: IdRef<ArmatureId>) -> Result<&Armature, RnaError> {
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
