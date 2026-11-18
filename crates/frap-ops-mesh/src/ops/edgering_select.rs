// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.edgering_select(delimit_edge_ring={'NGONS'}, extend=False, deselect=False,
//! toggle=False, object_index=-1, edge_index=-1, vert_index=-1, face_index=-1)`, tier T1
//! (`tests/golden/cases/edit/select/loop_select*.py`). Port of `edbm_select_ring_exec` and
//! `mouse_mesh_loop_edge_ring` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later); the shared pick logic is `loop_select::exec_impl`.
//!
//! Like `mesh.loop_select`, selects by index (`object_index` among the objects in Edit Mode,
//! `edge_index` in `mesh.edges` order): the edge ring through the edge (`BMW_EDGERING` with
//! `delimit_edge_ring`), or the face loop in face select mode. With a `SEAM`/`SHARP` delimiter,
//! picking again a ring selected up to its delimiters selects the full ring.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::loop_select::{Pick, exec_impl};
use super::select_more::common::{self, Ngons};

#[operator(
    poll = poll::EDITMESH_VIEW3D,
    tier = T1,
    notes = [
        "select by index like Blender's redo path: object_index (objects in Edit Mode, active first) and edge_index (mesh.edges order) are required; without them the operator is CANCELLED",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::EdgeringSelect) -> OpResult {
    let delimit = common::walk_delimit(&p.delimit_edge_ring.value, Ngons::Ring);
    let pick = Pick {
        extend: p.extend.value,
        deselect: p.deselect.value,
        toggle: p.toggle.value,
        object_index: p.object_index.value,
        edge_index: p.edge_index.value,
        vert_index: p.vert_index.value,
        face_index: p.face_index.value,
    };
    exec_impl(cx, &pick, true, delimit)
}
