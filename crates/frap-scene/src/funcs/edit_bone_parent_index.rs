// SPDX-License-Identifier: GPL-3.0-or-later
//! `EditBone.parent_index()` (Python-only, `_bpy_types.py` `_GenericBone`; M5+M6 stabilization), shared
//! by the three bone types: `edit_armature::family::parent_index`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};

use crate::{FuncArgs, Host};

#[rna_func(srna = "EditBone", func = "parent_index", krate = crate)]
fn edit_bone_parent_index(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    crate::edit_armature::family::parent_index(h, this, args)
}
