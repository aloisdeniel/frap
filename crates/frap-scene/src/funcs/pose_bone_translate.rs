// SPDX-License-Identifier: GPL-3.0-or-later
//! `PoseBone.translate()` (Python-only, `_bpy_types.py` `_GenericBone`; M5+M6 stabilization), shared
//! by the three bone types: `edit_armature::family::translate`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};

use crate::{FuncArgs, Host};

#[rna_func(srna = "PoseBone", func = "translate", krate = crate)]
fn pose_bone_translate(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    crate::edit_armature::family::translate(h, this, args)
}
