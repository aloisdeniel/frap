// SPDX-License-Identifier: GPL-3.0-or-later
//! `Armature.edit_bones` and `edit_bones.active` (WP 6.2, `rna_Armature_act_edit_bone_set`):
//! the edit bones of an armature in Edit Mode (`EditState::armatures`), an empty collection
//! otherwise (Blender's `arm->edbo` is `NULL`). Making a bone active also selects it
//! (`BONE_SELECTED`); a bone of another armature or `None` clears nothing but the active bone.

use std::sync::Arc;

use frap_rna::traits::VecColl;
use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::edit_armature::paths::{armature_of, edit_bone_of, edit_bone_ptr};
use crate::edit_armature::{EditBone, SEL};
use crate::{Host, HostRead};

const NO_BONES: &[EditBone] = &[];

#[rna_hook(srna = "Armature", prop = "edit_bones", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let arm = armature_of(this)?;
    let items: &'h [EditBone] = h
        .edit()
        .armatures
        .get(&arm.key())
        .map_or(NO_BONES, |ea| &ea.edit_bones);
    Ok(Field::Coll(CollRef::Owned(Box::new(VecColl::new(
        items,
        &frap_spec_rna::EDIT_BONE,
    )))))
}

/// `ArmatureEditBones` is the API struct of `Armature.edit_bones` (no binding of its own), so
/// its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ArmatureEditBones",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "ArmatureEditBones", prop = "active", krate = crate)]
mod active {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let arm = armature_of(this)?;
        let Some(ea) = h.edit().armatures.get(&arm.key()) else {
            return Ok(Field::Value(Value::None));
        };
        Ok(match ea.active.and_then(|a| ea.edit_bones.get(a)) {
            Some(b) => Field::Ptr(edit_bone_ptr(arm, &b.name)),
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
        let target = match &v {
            Value::Ptr(p) => match edit_bone_of(p) {
                Ok((a, key)) if a == arm => Some(key),
                // Another armature's bone: Blender silently ignores it.
                _ => return Ok(()),
            },
            _ => None,
        };
        let Some(ea) = h.edit_mut().armatures.get_mut(&arm.key()) else {
            return Ok(());
        };
        let ea = Arc::make_mut(ea);
        match target {
            None => ea.active = None,
            Some(key) => {
                if let Some(i) = key.find(ea.edit_bones.iter().map(|b| b.name.as_str())) {
                    ea.active = Some(i);
                    ea.edit_bones[i].select[SEL] = true;
                }
            }
        }
        Ok(())
    }
}
