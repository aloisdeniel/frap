// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.customdata_custom_splitnormals_clear()`, tier T1
//! (`tests/golden/cases/edit/normals/custom_*.py`). Port of
//! `mesh_customdata_custom_splitnormals_clear_exec` (`editors/mesh/mesh_data.cc`), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_editable_mesh`. Removes the `custom_normal` attribute (Object Mode: of any
//! type and domain; Edit Mode: the INT16_2D corner layer), so the mesh is shaded with its
//! automatic normals again. `CANCELLED` without one. The active attribute keeps its index, as
//! in Blender (the attribute after it becomes active when it came later in the list).

use frap_mesh::AttrData;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::clnor;
use super::geometry__attribute_add::{attrs, common};

#[operator(poll = poll::ACTIVE_EDITABLE_MESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::CustomdataCustomSplitnormalsClear) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let exists = common::read(cx, me, |s, _| match s.column(clnor::LAYER) {
        Some((_, AttrData::Int16x2(_))) => true,
        Some(_) => !s.is_edit(),
        None => false,
    });
    if exists != Some(true) {
        return Ok(OpStatus::Cancelled);
    }
    common::with_store(cx, me, |store, names, _| {
        let index = attrs::index_of(&store.layers(), names.active.as_deref());
        store.take(clnor::LAYER);
        if index >= 0 {
            names.active = attrs::name_at(&store.layers(), index, store.is_edit());
        }
    })?;
    Ok(OpStatus::Finished)
}
