// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.fill_holes(sides=4)`, tier T1 (`tests/golden/cases/edit/create/fill_holes.py`).
//! Port of `edbm_fill_holes_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `bmesh.ops.holes_fill`: every hole bounded by selected boundary
//! edges with at most `sides` corners (0: any) gets one face, added to the selection.

use frap_bmesh::BMElem;
use frap_bmesh::ops::holes_fill::{HolesFillIn, holes_fill};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::FillHoles) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().1) == Some(0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let edges = edbm::sel_edges(bm);
        let r = holes_fill(
            bm,
            &HolesFillIn {
                edges,
                sides: p.sides.value,
            },
        );
        if let Some(out) = edbm::finish(cx, r)? {
            let bm = cx.bmesh_mut(me)?;
            let faces: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
            edbm::select_enable(bm, &faces, true);
        }
    }
    Ok(OpStatus::Finished)
}
