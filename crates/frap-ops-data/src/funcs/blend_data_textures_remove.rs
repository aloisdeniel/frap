// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.textures.remove(texture, do_unlink=True, do_id_user=True, do_ui_user=True)`:
//! deletes the texture, clearing every reference to it (a DISPLACE modifier's `texture` reads
//! `None` afterwards) and releasing its image (verified on 5.2.4: the image's `users` drops back
//! to 0). The zero-users check of `do_unlink=False` is in `_common.rs`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

#[rna_func(srna = "BlendDataTextures", func = "remove")]
fn blend_data_textures_remove(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::remove(h, args, "texture")
}
