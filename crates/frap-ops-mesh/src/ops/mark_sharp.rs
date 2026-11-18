// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.mark_sharp(clear=False, use_verts=False)`, tier T1
//! (`tests/golden/cases/edit/normals/mark_*.py`). Port of `edbm_mark_sharp_exec`
//! (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Every mesh in Edit Mode (unique data) with a selected edge (`use_verts`: a selected vertex)
//! marks (or with `clear` unmarks) as sharp its selected edges (`use_verts`: every edge with a
//! selected vertex; hidden edges are not skipped, as in Blender). Leaving Edit Mode writes the
//! `sharp_edge` EDGE attribute while some edge is sharp and removes it when none is
//! ([MESH §2.5]). The operator always finishes.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{edit_meshes_where, sel};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::MarkSharp) -> OpResult {
    let clear = p.clear.value;
    let use_verts = p.use_verts.value;
    let meshes = edit_meshes_where(cx, |bm| {
        let (v, e, _) = sel(bm);
        if use_verts { v > 0 } else { e > 0 }
    });
    for me in meshes {
        let bm = cx.bmesh_mut(me)?;
        for e in bm.edge_ids() {
            let hit = if use_verts {
                let [a, b] = bm.edge(e).v;
                bm.vert(a).flag.contains(ElemFlag::SELECT)
                    || bm.vert(b).flag.contains(ElemFlag::SELECT)
            } else {
                bm.edge(e).flag.contains(ElemFlag::SELECT)
            };
            if hit {
                bm.edge_mut(e).flag.set(ElemFlag::SMOOTH, clear);
            }
        }
    }
    Ok(OpStatus::Finished)
}
