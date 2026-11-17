// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.cameras.new(name)`: camera data with the fresh values of `Camera::new`
//! (perspective, 50 mm lens, verified on 5.2.4), no users.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Camera;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataCameras", func = "new")]
fn blend_data_cameras_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let (main, log) = h.main_mut();
    Ok(main.cameras.insert(&name, Camera::new(&name), log).into())
}
