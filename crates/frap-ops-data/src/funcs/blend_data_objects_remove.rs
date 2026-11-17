// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.objects.remove(object, do_unlink=True, do_id_user=True, do_ui_user=True)`:
//! deletes the Object, clearing every reference to it (the verified rules, including the
//! zero-users check of `do_unlink=False`, are in `_common.rs`).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

#[rna_func(srna = "BlendDataObjects", func = "remove")]
fn blend_data_objects_remove(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::remove(h, args, "object")
}
