// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.duplicate(mode=1)`, tier T1 (`tests/golden/cases/edit/misc/duplicate_*.py`).
//! Port of `edbm_duplicate_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with a selected vertex (multi-object edit): `bmesh.ops.duplicate`
//! of the selected vertices, edges and faces (`use_select_history`: the history moves to the
//! copies; `use_edge_flip_from_face`: a copied boundary edge is wound like its face), then the
//! originals are deselected and the copies selected, keeping the remapped history. The copies
//! are appended (or fill freed slots) and are not moved; `mode` is only read by the
//! interactive transform that follows. Nothing selected anywhere: `CANCELLED`.

use frap_bmesh::ops::duplicate::{DuplicateIn, duplicate};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::Duplicate) -> OpResult {
    let mut changed = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (v, _, _) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if v == 0 {
            continue;
        }
        changed = true;
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::selected_geom(bm);
        let r = duplicate(
            bm,
            &DuplicateIn {
                geom,
                use_select_history: true,
                use_edge_flip_from_face: true,
                ..DuplicateIn::default()
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        // De-select all would clear the history otherwise (`BM_SELECT_HISTORY_BACKUP`).
        let history = std::mem::take(&mut bm.select_history);
        edbm::deselect_all(bm);
        edbm::select_elems(bm, &out.geom);
        bm.select_history = history;
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
