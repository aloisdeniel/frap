// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.scenes.new(name)` (`rna_Main_scenes_new` → `BKE_scene_add`, M3 WP 3.10): a scene
//! with the default settings, no objects, camera or world, and one view layer `ViewLayer`
//! (`frap_scene::scenes::new_scene`; verified on 5.2.4: the name is uniquified, `''` gives
//! `Scene`, the window keeps showing its scene, the new scene has one user).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataScenes", func = "new")]
fn blend_data_scenes_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let (main, log) = h.main_mut();
    Ok(frap_scene::scenes::new_scene(main, &name, log).into())
}
