// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.unsubdivide(iterations=2)`, tier T1 (`tests/golden/cases/edit/subdivide/
//! unsubdivide_*.py`). Port of `edbm_unsubdivide_exec` (`editors/mesh/editmesh_tools.cc`,
//! Blender 5.2.4, GPL-2.0-or-later) over `frap_bmesh::ops::unsubdivide`.
//!
//! The selected vertices are the candidates: grid-like regions lose every other vertex per
//! iteration (a cube subdivided once comes back to 8 vertices). Every object in Edit Mode
//! takes part, **shared meshes once per object** (Blender lists the objects, not their data);
//! a mesh with nothing selected is skipped. The selection is then flushed for the select mode
//! (through vertices first when vertex mode is off).

use frap_bmesh::{ElemFlag, SelectMode};
use frap_bmesh::ops::unsubdivide::{UnsubdivideIn, unsubdivide};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::loopcut::edit_mode_bases;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Unsubdivide) -> OpResult {
    let iterations = p.iterations.value;
    for (_, me) in edit_mode_bases(cx) {
        let Some(me) = me else { continue };
        let any = {
            let (_, edit) = cx.split_edit();
            edit.meshes.get(&me.key()).is_some_and(|em| {
                let (v, e, f) = em.bm.selected_counts();
                v + e + f > 0
            })
        };
        if !any {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let verts = bm
            .vert_ids()
            .into_iter()
            .filter(|&v| {
                let f = bm.vert(v).flag;
                f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN)
            })
            .collect();
        unsubdivide(bm, &UnsubdivideIn { verts, iterations })
            .map_err(|e| OpError::Runtime(e.to_string()))?;
        if !bm.select_mode.contains(SelectMode::VERT) {
            // Flush vertices to faces first.
            bm.select_flush_mode_ex(SelectMode::VERT, false);
        }
        bm.select_flush_mode();
    }
    Ok(OpStatus::Finished)
}
