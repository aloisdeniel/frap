// SPDX-License-Identifier: GPL-3.0-or-later
//! `Armature.collections` (the root bone collections) and its API struct `BoneCollections`
//! (`active`, `active_index`, `active_name`, `is_solo_active`), WP 6.2. Port of
//! `rna_iterator_bone_collections_roots_*`, `rna_BoneCollections_active_*`
//! (blender/source/blender/makesrna/intern/rna_armature.cc) and
//! `ANIM_armature_bonecoll_active_*` (blender/source/blender/animrig/intern/bone_collections.cc),
//! Blender 5.2.4, GPL-2.0-or-later. Managing collections (`new`, `remove`, `move`, the
//! `armature.collection_*` operators) arrives in M8.

use smol_str::SmolStr;
use frap_rna::traits::index_out_of_range;
use frap_rna::{
    CollRef, Field, Item, PropDef, RnaColl, RnaError, RnaPtr, StructDef, StructRef,
    Value, rna_hook,
};

use crate::edit_armature::paths::{armature, armature_of, collection_of, collection_ptr};
use crate::id::BoneCollection;
use crate::{Host, HostRead};

/// The root collections, in `collections_all` order.
struct Roots<'h> {
    items: Vec<&'h BoneCollection>,
}

impl RnaColl for Roots<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::BONE_COLLECTION
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        self.items.get(i).map(|c| c.name.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        self.items.iter().position(|c| c.name == key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.items
            .get(i)
            .map(|c| Item::Struct(StructRef::Borrowed(*c)))
            .ok_or_else(|| index_out_of_range(i, self.items.len()))
    }
}

#[rna_hook(srna = "Armature", prop = "collections", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let a = armature(h.main(), armature_of(this)?)?;
    let items = a.collections.iter().filter(|c| c.parent.is_none()).collect();
    Ok(Field::Coll(CollRef::Owned(Box::new(Roots { items }))))
}

/// The `BoneCollections` API struct has no binding of its own: its hook bindings are
/// declared here.
macro_rules! decl {
    ($name:ident, $prop:literal) => {
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
        #[linkme(crate = frap_rna::linkme)]
        static $name: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
            srna: "BoneCollections",
            prop: $prop,
            coll: false,
        };
    };
}
decl!(DECL_ACTIVE, "active");
decl!(DECL_ACTIVE_INDEX, "active_index");
decl!(DECL_ACTIVE_NAME, "active_name");
decl!(DECL_IS_SOLO_ACTIVE, "is_solo_active");

/// Sets the active collection (`ANIM_armature_bonecoll_active_index_set`: an index out of
/// range clears it).
fn set_active(h: &mut dyn Host, this: &RnaPtr, index: Option<usize>) -> Result<(), RnaError> {
    let arm = armature_of(this)?;
    let (main, log) = h.main_mut();
    if let Some(a) = main.armatures.get_mut(arm, log) {
        a.active_collection = index.filter(|&i| i < a.collections.len());
    }
    Ok(())
}

#[rna_hook(srna = "BoneCollections", prop = "active", krate = crate)]
mod active {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let arm = armature_of(this)?;
        let a = armature(h.main(), arm)?;
        Ok(match a.active_collection.and_then(|i| a.collections.get(i)) {
            Some(c) => Field::Ptr(collection_ptr(arm, &c.name)),
            None => Field::Value(Value::None),
        })
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let arm = armature_of(this)?;
        let index = match &v {
            Value::Ptr(p) => match collection_of(p) {
                Ok((a, key)) if a == arm => {
                    let names = armature(h.main(), arm)?
                        .collections
                        .iter()
                        .map(|c| c.name.to_string())
                        .collect::<Vec<_>>();
                    key.find(names.iter().map(|s| s.as_str()))
                }
                _ => None,
            },
            _ => None,
        };
        set_active(h, this, index)
    }
}

#[rna_hook(srna = "BoneCollections", prop = "active_index", krate = crate)]
mod active_index {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let a = armature(h.main(), armature_of(this)?)?;
        Ok(Field::Value(Value::Int(
            a.active_collection.map_or(-1, |i| i as i64),
        )))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let i = match v {
            Value::Int(i) => usize::try_from(i).ok(),
            _ => None,
        };
        set_active(h, this, i)
    }
}

#[rna_hook(srna = "BoneCollections", prop = "active_name", krate = crate)]
mod active_name {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let a = armature(h.main(), armature_of(this)?)?;
        let name = a
            .active_collection
            .and_then(|i| a.collections.get(i))
            .map(|c| c.name.clone())
            .unwrap_or_default();
        Ok(Field::Value(Value::Str(name)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let name = match &v {
            Value::Str(s) => s.clone(),
            _ => SmolStr::default(),
        };
        let i = armature(h.main(), armature_of(this)?)?
            .collections
            .iter()
            .position(|c| c.name == name);
        set_active(h, this, i)
    }
}

#[rna_hook(srna = "BoneCollections", prop = "is_solo_active", krate = crate)]
fn is_solo_active<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let a = armature(h.main(), armature_of(this)?)?;
    Ok(Field::Value(Value::Bool(
        a.collections.iter().any(|c| c.is_solo),
    )))
}
