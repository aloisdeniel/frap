// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.mark_seam(clear=False)`, tier T1 (`tests/golden/cases/edit/normals/
//! mark_*.py`). Port of `edbm_mark_seam_exec` (`editors/mesh/editmesh_tools.cc`), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! Every mesh in Edit Mode (unique data) with a selected edge marks (or with `clear` unmarks)
//! its selected, visible edges as UV seams. Leaving Edit Mode writes the `uv_seam` EDGE
//! attribute while some edge is a seam and removes it when none is ([MESH §2.5]). Live unwrap
//! (`tool_settings.use_edge_path_live_unwrap`, off by default) is not run. The operator
//! always finishes.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{edit_meshes_where, sel};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::MarkSeam) -> OpResult {
    let clear = p.clear.value;
    for me in edit_meshes_where(cx, |bm| sel(bm).1 > 0) {
        let bm = cx.bmesh_mut(me)?;
        for e in bm.edge_ids() {
            let f = bm.edge(e).flag;
            if !f.contains(ElemFlag::SELECT) || f.contains(ElemFlag::HIDDEN) {
                continue;
            }
            bm.edge_mut(e).flag.set(ElemFlag::SEAM, !clear);
        }
    }
    Ok(OpStatus::Finished)
}
