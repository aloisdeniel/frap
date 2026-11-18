// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.textures.new(name, type)`: a texture of the given type with Blender's fresh
//! values (`TextureFields::default()`, verified on 5.2.4: `extension` `'REPEAT'`, `intensity`
//! 1, `use_color_ramp` off), no users. `type` is required and checked against the twelve
//! texture types by the argument binding (`BlendDataTextures.new(): required parameter "type"
//! not specified`, `enum "BOGUS" not found in ('NONE', 'BLEND', …)`); `'IMAGE'` gives an
//! `ImageTexture`, `'NONE'` a plain `Texture`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Texture;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataTextures", func = "new")]
fn blend_data_textures_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let ty = arg_str(args, "type").to_owned();
    let (main, log) = h.main_mut();
    Ok(main
        .textures
        .insert(&name, Texture::with_type(&name, &ty), log)
        .into())
}
