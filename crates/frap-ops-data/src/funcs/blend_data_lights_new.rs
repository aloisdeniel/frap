// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.lights.new(name, type)`: light data of the given type with the fresh values of
//! `Light::new` (verified on 5.2.4 for every type: 10 W, white, `shadow_soft_size` 0; `type`
//! is required and checked against `POINT`/`SUN`/`SPOT`/`AREA` by the argument binding).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Light;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataLights", func = "new")]
fn blend_data_lights_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let ty = arg_str(args, "type").to_owned();
    let (main, log) = h.main_mut();
    Ok(main
        .lights
        .insert(&name, Light::new(&name, &ty), log)
        .into())
}
