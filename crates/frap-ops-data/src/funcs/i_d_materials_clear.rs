// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.materials.clear()` (`rna_IDMaterials_clear_id` → `BKE_id_material_clear`): removes
//! every slot and the mesh's `material_index` attribute (verified on 5.2.4); objects using the
//! mesh lose their per-slot entries (`BKE_object_material_resize(…, 0)`).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of, resize_object_slots};

#[rna_func(srna = "IDMaterials", func = "clear")]
fn i_d_materials_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let me = mesh_of(this)?;
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("materials.clear(): the mesh was removed"))?;
    let old = mesh.materials.len();
    mesh.materials.clear();
    if mesh.attributes().get("material_index").is_some() {
        mesh.attributes_mut().remove("material_index");
    }
    resize_object_slots(main, me.any(), old, 0, log);
    Ok(Value::None)
}
