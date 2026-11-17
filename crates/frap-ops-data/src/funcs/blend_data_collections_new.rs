// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.collections.new(name)`: an empty collection linked nowhere (verified on 5.2.4:
//! `users == 0` until `scene.collection.children.link(…)`).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Collection;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataCollections", func = "new")]
fn blend_data_collections_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let (main, log) = h.main_mut();
    Ok(main
        .collections
        .insert(&name, Collection::new(name.as_str()), log)
        .into())
}
