// SPDX-License-Identifier: GPL-3.0-or-later
//! `Armature.bones.active` (WP 6.2, `rna_Armature_act_bone_set`): the active rest bone
//! (`act_bone`). Assigning a bone of this armature makes it active and selects it
//! (`BONE_SELECTED`); `None` clears it; a bone of another armature is ignored (Blender prints
//! an error and changes nothing).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::edit_armature::paths::{armature, armature_of, bone_of, bone_ptr};
use crate::edit_armature::SEL;
use crate::{Host, HostRead};

/// `ArmatureBones` is the API struct of `Armature.bones` (no binding of its own), so its hook
/// binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ArmatureBones",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "ArmatureBones", prop = "active", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let arm = armature_of(this)?;
        let a = armature(h.main(), arm)?;
        Ok(match a.active_bone.and_then(|i| a.bones.get(i)) {
            Some(b) => Field::Ptr(bone_ptr(arm, &b.name)),
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
            Value::Ptr(p) => match bone_of(p) {
                Ok((a, key)) if a == arm => Some(key),
                _ => return Ok(()),
            },
            _ => None,
        };
        let (main, log) = h.main_mut();
        let Some(a) = main.armatures.get_mut(arm, log) else {
            return Ok(());
        };
        match target {
            None => a.active_bone = None,
            Some(key) => {
                if let Some(i) = key.find(a.bones.iter().map(|b| b.name.as_str())) {
                    a.active_bone = Some(i);
                    a.bones[i].select[SEL] = true;
                }
            }
        }
        Ok(())
    }
}
