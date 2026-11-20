// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.armatures.new(name)` (WP 6.2): an armature without bones or bone collections,
//! with `armature_init_data`'s values (`OCTAHEDRAL` display, `POSE` position; verified on
//! 5.2.4), no users.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Armature;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataArmatures", func = "new")]
fn blend_data_armatures_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let (main, log) = h.main_mut();
    Ok(main
        .armatures
        .insert(&name, Armature::new(&name), log)
        .into())
}
