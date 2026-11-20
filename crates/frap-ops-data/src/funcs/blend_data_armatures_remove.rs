// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.armatures.remove(armature, do_unlink=True, do_id_user=True, do_ui_user=True)`
//! (WP 6.2): deletes the Armature and the objects using it (`BKE_id_delete`; the verified
//! rules, including the zero-users check of `do_unlink=False`, are in `_common.rs`), and drops
//! its edit bones if it was in Edit Mode.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::arg_id;

#[rna_func(srna = "BlendDataArmatures", func = "remove")]
fn blend_data_armatures_remove(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let key = arg_id(args, "armature").map(|a| a.key);
    let v = super::_common::remove(h, args, "armature")?;
    if let Some(k) = key {
        h.edit_mut().armatures.remove(&k);
    }
    Ok(v)
}
