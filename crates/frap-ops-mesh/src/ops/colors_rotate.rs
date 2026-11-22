// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.colors_rotate(use_ccw=False)`, tier T0
//! (`tests/golden/cases/edit/normals_m8/uvs_colors*.py`). Port of `edbm_rotate_colors_exec`
//! (`editors/mesh/editmesh_tools.cc`) and `bmo_rotate_colors_exec` (`bmesh/operators/
//! bmo_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data) with a selected face whose **active colour
//! attribute** (`color_attributes.active_color`) is a face-corner colour (`FLOAT_COLOR` or
//! `BYTE_COLOR`), its values move one corner along each selected face, as `mesh.uvs_rotate`
//! moves UVs. A mesh whose active colour is unset or on the vertices is left alone. The operator
//! always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{edit_meshes_where, sel};
use super::uvs_rotate::cycle;

/// Applies `op` to the active corner colour of every mesh in Edit Mode with a selected face.
pub(crate) fn colors_exec(cx: &mut OpCtx, op: cycle::Cycle) -> OpResult {
    for me in edit_meshes_where(cx, |bm| sel(bm).2 > 0) {
        let name = active_color_name(cx, me);
        let bm = cx.bmesh_mut(me)?;
        if let Some(layer) = cycle::active_corner_color(bm, name.as_deref()) {
            cycle::apply(bm, layer, op);
        }
    }
    Ok(OpStatus::Finished)
}

/// `BKE_object_get_original_mesh(ob)->active_color_attribute`.
fn active_color_name(cx: &OpCtx, me: IdRef<MeshId>) -> Option<String> {
    cx.main()
        .meshes
        .get(me)
        .and_then(|m| m.color_active.as_ref())
        .map(|s| s.to_string())
}

#[operator(poll = poll::EDITMESH, tier = T0)]
fn exec(cx: &mut OpCtx, p: mesh::ColorsRotate) -> OpResult {
    colors_exec(
        cx,
        cycle::Cycle::Rotate {
            use_ccw: p.use_ccw.value,
        },
    )
}
