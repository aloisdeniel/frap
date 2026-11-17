// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.objects.new(name, object_data)` (`rna_Main_objects_new`), verified on 5.2.4:
//! the object gets the lowest free name (255 bytes, `''` → `Object`), an identity transform and
//! the fresh values of `Object::new`; it is linked to **no** collection (`users == 0`, not in
//! any view layer). `object_data=None` makes an EMPTY; a mesh, camera, light, armature or
//! point cloud gives the matching type and one more user of the data; any other ID raises
//! `RuntimeError: Error: ID type 'MATERIAL' is not valid for an object`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Object;
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_id, arg_str, id_type_identifier, is_obdata, report_error};

#[rna_func(srna = "BlendDataObjects", func = "new")]
fn blend_data_objects_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let data = arg_id(args, "object_data");
    if let Some(d) = data
        && !is_obdata(d.code)
    {
        return Err(report_error(format!(
            "ID type '{}' is not valid for an object",
            id_type_identifier(d.code)
        )));
    }
    let name = arg_str(args, "name").to_owned();
    let (main, log) = h.main_mut();
    let r = main.objects.insert(&name, Object::new(&name, data), log);
    Ok(r.into())
}
