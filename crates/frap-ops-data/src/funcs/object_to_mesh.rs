// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.to_mesh(preserve_all_data_layers=False, depsgraph=None)` (`rna_Object_to_mesh` →
//! `BKE_object_to_mesh`). Verified on 5.2.4: an object without geometry (empty, camera,
//! light) raises `RuntimeError: Error: Object does not have geometry data`; on a mesh object it
//! returns a temporary mesh owned by the object (named after the data, `users == 0`, not in
//! `bpy.data.meshes`): the evaluated geometry for an evaluated object, the undeformed data for
//! an original one.
//!
//! frap has no evaluated IDs (`evaluated_get` returns the original, see `i_d_evaluated_get.rs`),
//! so it returns the object's mesh datablock itself when the evaluated mesh *is* that data
//! (no modifier, shape key or edit-mode change: the depsgraph-lite shares the original `Arc`).
//! When the evaluated geometry differs, a separate result pointer is needed (M4, with the
//! modifier evaluators) and frap raises `NotImplementedError` (`RNA_NOT_IMPLEMENTED`) instead
//! of returning wrong data.

use std::sync::Arc;

use frap_rna::{IdCode, IdRef, MeshId, ObjectId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{EvalMode, FuncArgs, Host};

use super::_common::{internal, not_implemented, report_error, root_id};

#[rna_func(srna = "Object", func = "to_mesh")]
fn object_to_mesh(h: &mut dyn Host, this: &RnaPtr, _args: &mut FuncArgs) -> Result<Value, RnaError> {
    let ob: IdRef<ObjectId> = root_id(this)?
        .typed()
        .ok_or_else(|| internal("to_mesh(): not an object"))?;
    let data = h
        .main()
        .objects
        .get(ob)
        .ok_or_else(|| internal("to_mesh(): the object was removed"))?
        .data;
    let me: IdRef<MeshId> = match data {
        Some(d) if d.code == IdCode::ME => d.typed().ok_or_else(|| internal("bad mesh key"))?,
        _ => return Err(report_error("Object does not have geometry data")),
    };
    let original = h
        .main()
        .meshes
        .get_arc(me)
        .ok_or_else(|| internal("to_mesh(): the mesh was removed"))?;
    let (host, ev) = h.eval();
    let evaluated = ev
        .object(host, ob, EvalMode::Viewport)
        .map_err(|e| report_error(e.message))?;
    match &evaluated.mesh {
        Some(m) if Arc::ptr_eq(m, &original) => Ok(me.into()),
        _ => Err(not_implemented(
            "to_mesh() of an object whose evaluated mesh differs from its data (modifiers, \
             shape keys, Edit Mode) needs evaluated-mesh pointers, which arrive with M4",
        )),
    }
}
