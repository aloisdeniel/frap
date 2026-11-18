// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.customdata_custom_splitnormals_add()`, tier T1
//! (`tests/golden/cases/edit/normals/custom_*.py`). Port of
//! `mesh_customdata_custom_splitnormals_add_exec` (`editors/mesh/mesh_data.cc`), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_editable_mesh` (the active object's mesh, Object or Edit Mode). Adds the
//! `custom_normal` INT16_2D CORNER attribute filled with zeros (automatic normals: the shading
//! does not change, but `has_custom_normals` is true and `normals_domain` becomes `CORNER`).
//! In Object Mode it is appended to the attribute list; in Edit Mode it is a BMesh layer.
//! `CANCELLED` when the mesh already has a `custom_normal` attribute.

use frap_mesh::Domain;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::clnor;
use super::geometry__attribute_add::{attrs, common};

#[operator(poll = poll::ACTIVE_EDITABLE_MESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::CustomdataCustomSplitnormalsAdd) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let exists = common::read(cx, me, |s, _| s.column(clnor::LAYER).is_some());
    if exists != Some(false) {
        return Ok(OpStatus::Cancelled);
    }
    common::with_store(cx, me, |store, _, _| {
        attrs::add_default(store, clnor::LAYER.into(), Domain::Corner, "INT16_2D")
    })?;
    Ok(OpStatus::Finished)
}
