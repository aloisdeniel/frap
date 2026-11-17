// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.materials.append(material)` (`rna_IDMaterials_append_id` → `BKE_id_material_append`):
//! appends a slot (`None` appends an empty one, verified on 5.2.4). Every object using the
//! mesh sees the new slot at once (slots are derived, §5.6); their per-object entries follow
//! Blender's resize rule (`_common::resize_object_slots`).

use frap_rna::{IdRef, MaterialId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_id, internal, mesh_of, resize_object_slots};

#[rna_func(srna = "IDMaterials", func = "append")]
fn i_d_materials_append(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "append")?;
    let me = mesh_of(this)?;
    let ma: Option<IdRef<MaterialId>> = arg_id(args, "material").and_then(|a| a.typed());
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("materials.append(): the mesh was removed"))?;
    let old = mesh.materials.len();
    mesh.materials.push(ma);
    resize_object_slots(main, me.any(), old, old + 1, log);
    Ok(Value::None)
}
