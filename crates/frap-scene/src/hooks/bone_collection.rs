// SPDX-License-Identifier: GPL-3.0-or-later
//! `BoneCollection` hooks (WP 6.2): `name` (`ANIM_armature_bonecoll_name_set`: an empty name
//! becomes `Bones`, then unique with `.NNN`, and the bones assigned by name follow), `is_visible`,
//! `is_solo`, and the derived `bones`, `children`, `parent`, `index`, `is_visible_ancestors`,
//! `is_visible_effectively` (blender/source/blender/makesrna/intern/rna_armature.cc,
//! blender/source/blender/animrig/intern/bone_collections.cc, Blender 5.2.4,
//! GPL-2.0-or-later). The writes are set hooks so that collections reached through
//! `Armature.collections` (a hook collection) can be written too.
//!
//! `bones` in Edit Mode is empty (Blender adds the WARNING `Collection.bones` is not available
//! in armature edit mode, which a read cannot report).

use std::sync::Arc;

use smol_str::SmolStr;
use frap_rna::traits::FromRnaValue;
use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::edit_armature::paths::{
    ItemKey, armature, collection_of, collection_ptr, internal_error, ref_coll,
};
use crate::edit_armature::{collection_visible_effectively, collection_visible_with_ancestors};
use crate::id::Armature;
use crate::{Host, HostRead};

/// Index of the collection a pointer names.
fn index_of(a: &Armature, key: &ItemKey) -> Result<usize, RnaError> {
    key.find(a.collections.iter().map(|c| c.name.as_str()))
        .ok_or_else(|| internal_error("the bone collection was removed"))
}

fn located<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<(&'h Armature, usize), RnaError> {
    let (arm, key) = collection_of(this)?;
    let a = armature(h.main(), arm)?;
    Ok((a, index_of(a, &key)?))
}

#[rna_hook(srna = "BoneCollection", prop = "name", krate = crate)]
fn set_name(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (arm, key) = collection_of(this)?;
    let requested = SmolStr::from_rna_value(prop, v)?;
    let edit_key = arm.key();
    let (old, new) = {
        let a = armature(h.main(), arm)?;
        let i = index_of(a, &key)?;
        let new = frap_rna::naming::unique_subname(
            &|n| {
                a.collections
                    .iter()
                    .enumerate()
                    .any(|(j, c)| j != i && c.name == n)
            },
            &requested,
            crate::edit_armature::names::MAX_BONE_NAME,
            "Bones",
        );
        (a.collections[i].name.clone(), new)
    };
    if old == new {
        return Ok(());
    }
    let rename = |names: &mut Vec<SmolStr>| {
        for n in names.iter_mut() {
            if *n == old {
                *n = new.clone();
            }
        }
    };
    {
        let (main, log) = h.main_mut();
        if let Some(a) = main.armatures.get_mut(arm, log) {
            for c in &mut a.collections {
                if c.name == old {
                    c.name = new.clone();
                }
            }
            for b in &mut a.bones {
                rename(&mut b.collections);
            }
        }
    }
    if let Some(ea) = h.edit_mut().armatures.get_mut(&edit_key) {
        for b in &mut Arc::make_mut(ea).edit_bones {
            rename(&mut b.collections);
        }
    }
    Ok(())
}

/// Writes a boolean flag of one collection.
fn set_flag(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
    f: fn(&mut crate::id::BoneCollection, bool),
) -> Result<(), RnaError> {
    let (arm, key) = collection_of(this)?;
    let on = bool::from_rna_value(prop, v)?;
    let i = index_of(armature(h.main(), arm)?, &key)?;
    let (main, log) = h.main_mut();
    if let Some(a) = main.armatures.get_mut(arm, log) {
        f(&mut a.collections[i], on);
    }
    Ok(())
}

#[rna_hook(srna = "BoneCollection", prop = "is_visible", krate = crate)]
fn set_is_visible(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_flag(h, this, prop, v, |c, on| c.is_visible = on)
}

#[rna_hook(srna = "BoneCollection", prop = "is_solo", krate = crate)]
fn set_is_solo(
    h: &mut dyn Host,
    this: &RnaPtr,
    prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_flag(h, this, prop, v, |c, on| c.is_solo = on)
}

#[rna_hook(srna = "BoneCollection", prop = "bones", krate = crate)]
fn get_bones<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let (arm, _) = collection_of(this)?;
    let (a, i) = located(h, this)?;
    if h.edit().armatures.contains_key(&arm.key()) {
        // Blender's iterator reports a WARNING and yields nothing; a read hook has no report
        // channel, so frap answers the empty collection only.
        return Ok(ref_coll::<crate::id::Bone>(Vec::new(), &frap_spec_rna::BONE));
    }
    let name = &a.collections[i].name;
    Ok(ref_coll(
        a.bones.iter().filter(|b| b.collections.contains(name)).collect(),
        &frap_spec_rna::BONE,
    ))
}

#[rna_hook(srna = "BoneCollection", prop = "children", krate = crate)]
fn get_children<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let (arm, _) = collection_of(this)?;
    let (a, i) = located(h, this)?;
    let _ = arm;
    Ok(ref_coll(
        a.collections.iter().filter(|c| c.parent == Some(i)).collect(),
        &frap_spec_rna::BONE_COLLECTION,
    ))
}

#[rna_hook(srna = "BoneCollection", prop = "parent", krate = crate)]
fn get_parent<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let (arm, _) = collection_of(this)?;
    let (a, i) = located(h, this)?;
    Ok(match a.collections[i].parent.and_then(|p| a.collections.get(p)) {
        Some(p) => Field::Ptr(collection_ptr(arm, &p.name)),
        None => Field::Value(Value::None),
    })
}

#[rna_hook(srna = "BoneCollection", prop = "index", krate = crate)]
fn get_index<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let (_, i) = located(h, this)?;
    Ok(Field::Value(Value::Int(i as i64)))
}

#[rna_hook(srna = "BoneCollection", prop = "is_visible_ancestors", krate = crate)]
fn get_is_visible_ancestors<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let (a, i) = located(h, this)?;
    let visible = match a.collections[i].parent {
        Some(p) => collection_visible_with_ancestors(a, p),
        None => true,
    };
    Ok(Field::Value(Value::Bool(visible)))
}

#[rna_hook(srna = "BoneCollection", prop = "is_visible_effectively", krate = crate)]
fn get_is_visible_effectively<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let (a, i) = located(h, this)?;
    Ok(Field::Value(Value::Bool(collection_visible_effectively(a, i))))
}
