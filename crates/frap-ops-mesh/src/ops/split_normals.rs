// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.split_normals()`, tier T1
//! (`tests/golden/cases/edit/normals_m8/merge_split.py`). Port of `edbm_split_normals_exec`,
//! `normals_split_merge` and `normals_split` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data): the selected faces are made flat and the selected
//! edges sharp, the normal spaces are rebuilt, and every corner at a selected vertex gets the
//! normalised sum of the face normals of its smooth fan (walked over the smooth manifold edges
//! only, ignoring face smoothness), a corner between two sharp edges its face normal. The
//! `custom_normal` layer is created when missing. The operator always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::edit_meshes_where;
use super::merge_normals::split_merge;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::SplitNormals) -> OpResult {
    for me in edit_meshes_where(cx, |_| true) {
        split_merge(cx.bmesh_mut(me)?, false);
    }
    Ok(OpStatus::Finished)
}
