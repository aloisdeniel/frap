// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.colors_reverse()`, tier T0
//! (`tests/golden/cases/edit/normals_m8/uvs_colors*.py`). Port of `edbm_reverse_colors_exec`
//! (`editors/mesh/editmesh_tools.cc`) and `bm_face_reverse_colors` / `bmo_reverse_colors_exec`
//! (`bmesh/operators/bmo_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data) with a selected face whose active colour attribute
//! is a face-corner colour, the colours of each selected face are reversed as `mesh.uvs_reverse`
//! reverses UVs. A mesh whose active colour is unset or on the vertices is left alone. The
//! operator always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::colors_rotate::colors_exec;
use super::uvs_rotate::cycle;

#[operator(poll = poll::EDITMESH, tier = T0)]
fn exec(cx: &mut OpCtx, _p: mesh::ColorsReverse) -> OpResult {
    colors_exec(cx, cycle::Cycle::Reverse)
}
