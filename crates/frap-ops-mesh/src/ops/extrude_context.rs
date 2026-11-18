// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_context(use_normal_flip=False, use_dissolve_ortho_edges=False)`, tier
//! T1 (`tests/golden/cases/edit/extrude/context_*.py`). Port of `edbm_extrude_context_exec`
//! (`editors/mesh/editmesh_extrude.cc`, Blender 5.2.4, GPL-2.0-or-later): in 5.2 it runs the
//! same `edbm_extrude_mesh` as `mesh.extrude_region` (region, edges alone or vertices alone
//! depending on the select mode and selection; see `extrude_region.rs`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::extrude_region::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::ExtrudeContext) -> OpResult {
    common::extrude_mesh_all(
        cx,
        p.use_normal_flip.value,
        p.use_dissolve_ortho_edges.value,
    )?;
    Ok(OpStatus::Finished)
}
