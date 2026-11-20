// SPDX-License-Identifier: GPL-3.0-or-later
//! `VertexGroup.remove(index)` (`rna_VertexGroup_vertex_remove` → `vgroup_vert_remove`,
//! WP 6.3): removes the listed vertices from the group (the last weight of a vertex moves into
//! the hole, `BKE_defvert_remove_group`). In Edit Mode `RuntimeError: Error:
//! VertexGroup.remove(): cannot be called while object is in edit mode`. An index out of range
//! is skipped (Blender reads past the array).

use frap_bmesh::shape::deform;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, report_error};
use super::vertex_group_add::arg_indices;
use super::vertex_groups_new::{group_of, in_edit_mode, mesh_of_object};

#[rna_func(srna = "VertexGroup", func = "remove")]
fn vertex_group_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let ob = super::vertex_groups_new::object_of(this)?;
    let Some(me) = mesh_of_object(h.main(), ob) else {
        return Ok(Value::None);
    };
    if in_edit_mode(h, me) {
        return Err(report_error(
            "VertexGroup.remove(): cannot be called while object is in edit mode",
        ));
    }
    let indices = arg_indices(args);
    let Some((_, g)) = group_of(h.main(), this) else {
        return Ok(Value::None);
    };
    if h.main()
        .meshes
        .get(me)
        .and_then(|m| m.deform_verts())
        .is_none()
    {
        return Ok(Value::None);
    }
    let (main, log) = h.main_mut();
    let m = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("VertexGroup.remove(): the mesh was removed"))?;
    let n = m.verts_num();
    let dverts = m.deform_verts_mut();
    for i in indices {
        if let Some(dv) = usize::try_from(i)
            .ok()
            .filter(|&i| i < n)
            .map(|i| &mut dverts[i])
        {
            deform::remove_group(dv, g as u32);
        }
    }
    Ok(Value::None)
}
