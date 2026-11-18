// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.to_mesh(preserve_all_data_layers=False, depsgraph=None)` (`rna_Object_to_mesh` →
//! `BKE_object_to_mesh`). Verified on 5.2.4: an object without geometry (empty, camera,
//! light) raises `RuntimeError: Error: Object does not have geometry data`; on a mesh object it
//! returns a temporary mesh owned by the object (named after the data, `users == 0`, not in
//! `bpy.data.meshes`): the evaluated geometry for an evaluated object, the undeformed data for
//! an original one.
//!
//! Verified: writes to the result leave the object's mesh untouched
//! (`ob.to_mesh().vertices[0].co.x = 42` keeps `ob.data.vertices[0].co.x`), and the result
//! reads `is_evaluated == False` (also from an evaluated object).
//!
//! frap has no temporary meshes of its own: for an original object, or when the evaluated mesh
//! *is* the data (no modifier: the depsgraph-lite shares the original `Arc`), it returns the
//! object's mesh datablock behind a [`Detached::Temporary`] pointer (reads see the mesh, or
//! its edit mesh in Edit Mode). For an evaluated object with modifiers (WP 4.1) it returns
//! `bpy.data.objects['X'].data` behind a [`Detached::Temporary`] pointer, which
//! frap-session's `with_struct` reads as the object's evaluated mesh, always fresh. The
//! session refuses writes through either pointer, which would change the original
//! (deviation: Blender accepts them on the copy; `users` reads the original's count instead
//! of 0, and the result survives `to_mesh_clear()`; deviations.md D-41).

use std::sync::Arc;

use frap_rna::{Detached, IdCode, IdRef, MeshId, ObjectId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{EvalMode, FuncArgs, Host};

use super::_common::{detached, internal, not_implemented, report_error, root_id};

#[rna_func(srna = "Object", func = "to_mesh")]
fn object_to_mesh(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
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
    let data_ptr = || {
        Value::Ptr(Box::new(detached(
            RnaPtr::id(me.any(), &frap_spec_rna::MESH),
            Detached::Temporary,
        )))
    };
    // An original object gives its undeformed data (the edit mesh in Edit Mode, which the
    // data pointer reads), whatever its modifiers.
    if this.detached != Some(Detached::Evaluated) {
        return Ok(data_ptr());
    }
    let (host, ev) = h.eval();
    let evaluated = ev
        .object(host, ob, EvalMode::Viewport)
        .map_err(|e| report_error(e.message))?;
    match &evaluated.mesh {
        Some(m) if Arc::ptr_eq(m, &original) => Ok(data_ptr()),
        // WP 4.1: the evaluated mesh (modifiers) is read through `objects['X'].data`
        // (frap-session's `with_struct` evaluates it, always fresh).
        Some(_) => Ok(Value::Ptr(Box::new(detached(
            RnaPtr::id(ob.any(), &frap_spec_rna::OBJECT)
                .child(frap_rna::Seg::Prop("data"), &frap_spec_rna::MESH),
            Detached::Temporary,
        )))),
        None => Err(not_implemented(
            "to_mesh() of an object without an evaluated mesh",
        )),
    }
}
