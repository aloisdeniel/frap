// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.meshes.new(name)`: an empty mesh without any attribute (`Mesh::new`, verified on
//! 5.2.4), named with the lowest free `.NNN` (`''` → `Mesh`), with no users.

use frap_mesh::Mesh;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataMeshes", func = "new")]
fn blend_data_meshes_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let (main, log) = h.main_mut();
    Ok(main.meshes.insert(&name, Mesh::new(name.as_str()), log).into())
}
