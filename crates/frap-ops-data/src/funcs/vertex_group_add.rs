// SPDX-License-Identifier: GPL-3.0-or-later
//! `VertexGroup.add(index, weight, type)` (`rna_VertexGroup_vertex_add` → `vgroup_vert_add`,
//! WP 6.3): assigns `weight` (0..1, clamped by RNA) to every listed vertex with the assign
//! mode: `REPLACE` sets it, `ADD` adds and clamps at 1, `SUBTRACT` subtracts and removes the
//! vertex from the group at 0 or below (an unassigned vertex stays unassigned). Indices out of
//! range are skipped. Verified on 5.2.4 (`tests/golden/cases/weights_shapes/vgroup_add_modes.py`);
//! in Edit Mode `RuntimeError: Error: VertexGroup.add(): cannot be called while object is in
//! edit mode`; `type` is required.

use frap_bmesh::shape::deform::{self, AssignMode};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, internal, report_error};
use super::vertex_groups_new::{group_of, in_edit_mode, mesh_of_object};

/// The `index` list argument (an int, or a sequence of ints).
pub(crate) fn arg_indices(args: &FuncArgs) -> Vec<i64> {
    match args.get("index") {
        Some(Value::IntArray(v)) => v.to_vec(),
        Some(Value::Int(i)) => vec![*i],
        _ => Vec::new(),
    }
}

#[rna_func(srna = "VertexGroup", func = "add")]
fn vertex_group_add(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "add")?;
    let ob = super::vertex_groups_new::object_of(this)?;
    let Some(me) = mesh_of_object(h.main(), ob) else {
        return Ok(Value::None);
    };
    if in_edit_mode(h, me) {
        return Err(report_error(
            "VertexGroup.add(): cannot be called while object is in edit mode",
        ));
    }
    let mode = AssignMode::from_identifier(arg_str(args, "type"))
        .ok_or_else(|| internal("VertexGroup.add(): type was not converted"))?;
    let weight = args.get("weight").and_then(Value::as_f32).unwrap_or(0.0);
    let indices = arg_indices(args);
    let Some((_, g)) = group_of(h.main(), this) else {
        return Ok(Value::None);
    };
    let (main, log) = h.main_mut();
    let m = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("VertexGroup.add(): the mesh was removed"))?;
    let n = m.verts_num();
    let dverts = m.deform_verts_mut();
    for i in indices {
        if let Some(dv) = usize::try_from(i)
            .ok()
            .filter(|&i| i < n)
            .map(|i| &mut dverts[i])
        {
            deform::vert_add(dv, g as u32, weight, mode);
        }
    }
    Ok(Value::None)
}
