// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.materials.pop(index=-1)` (`rna_IDMaterials_pop_id` → `BKE_id_material_pop`): removes
//! slot `index` (negative counts from the end) and returns its material (`None` for an empty
//! slot). Verified on 5.2.4: an index outside the slots raises `RuntimeError: Error: Index out
//! of range` (also on an empty list); face `material_index` values above the removed slot are
//! decremented; objects using the mesh keep their per-slot entries by position (an `OBJECT`
//! slot 1 stays slot 1).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{
    arg_int, internal, material_index_remove, mesh_of, report_error, resize_object_slots,
};

#[rna_func(srna = "IDMaterials", func = "pop")]
fn i_d_materials_pop(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let me = mesh_of(this)?;
    let (main, log) = h.main_mut();
    let old = main
        .meshes
        .get(me)
        .ok_or_else(|| internal("materials.pop(): the mesh was removed"))?
        .materials
        .len();
    let mut index = arg_int(args, "index", -1);
    if index < 0 {
        index += old as i64;
    }
    if index < 0 || index >= old as i64 {
        return Err(report_error("Index out of range"));
    }
    let index = index as usize;
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("materials.pop(): the mesh was removed"))?;
    let ma = mesh.materials.remove(index);
    material_index_remove(mesh, index);
    resize_object_slots(main, me.any(), old, old - 1, log);
    Ok(Value::Id(ma.map(|m| m.any())))
}
