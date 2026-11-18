// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.delete(type='VERT')`, tier T1 (`tests/golden/cases/edit/delete/delete_*.py`).
//! Port of `edbm_delete_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with something of the right kind selected, `bmesh.ops.delete`
//! runs on the selected (visible) elements with the context of `type`:
//! - `VERT` (`VERTS`): the vertices and every edge and face using them;
//! - `EDGE` (`EDGES`): the edges, their faces, and vertices left without edges;
//! - `FACE` (`FACES`): the faces, then the edges and vertices no unselected face uses;
//! - `EDGE_FACE` (`EDGES_FACES`): the selected edges (with their faces) and selected faces,
//!   vertices stay;
//! - `ONLY_FACE` (`FACES_ONLY`): the faces only.
//!
//! Then everything is deselected (the selection history is cleared). Freed element slots are
//! reused LIFO by the next operator that creates geometry, as in Blender. Without a matching
//! selection on any mesh the operator is `CANCELLED` (Blender is silent; frap adds a WARNING,
//! D-51). `type` is `HIDDEN` and `SKIP_SAVE` in Blender but always settable.

pub(crate) mod common;

use frap_bmesh::BMElem;
use frap_bmesh::ops::delete::{DeleteIn, delete};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, delete::Type};

use common::{Removed, bm_err, edit_bm, sel_edges, sel_faces, sel_verts, totals};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "type is HIDDEN in Blender's UI but settable: VERT (default) | EDGE | FACE | EDGE_FACE | ONLY_FACE",
        "everything is deselected afterwards; result.removed / removed_edges / removed_faces count the deleted elements",
        "CANCELLED (with a WARNING) when no edit mesh has a matching selection",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::Delete) -> OpResult {
    let ty = p.r#type.value;
    let mut removed = Removed::default();
    let mut changed = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let Some(bm) = edit_bm(cx, me) else {
            continue;
        };
        let (tv, te, tf) = bm.selected_counts();
        let (geom, context): (Vec<BMElem>, &'static str) = match ty {
            Type::Vert if tv > 0 => (sel_verts(bm).into_iter().map(BMElem::Vert).collect(), "VERTS"),
            Type::Edge if te > 0 => (sel_edges(bm).into_iter().map(BMElem::Edge).collect(), "EDGES"),
            Type::Face if tf > 0 => (sel_faces(bm).into_iter().map(BMElem::Face).collect(), "FACES"),
            Type::EdgeFace if te > 0 || tf > 0 => {
                let mut g: Vec<BMElem> = sel_edges(bm).into_iter().map(BMElem::Edge).collect();
                g.extend(sel_faces(bm).into_iter().map(BMElem::Face));
                (g, "EDGES_FACES")
            }
            Type::OnlyFace if tf > 0 => {
                (sel_faces(bm).into_iter().map(BMElem::Face).collect(), "FACES_ONLY")
            }
            _ => continue,
        };
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        delete(bm, &DeleteIn { geom, context }).map_err(bm_err)?;
        changed = true;
        bm.deselect_all();
        removed.add(old, totals(bm));
    }
    if !changed {
        let what = match ty {
            Type::Vert => "vertices",
            Type::Edge => "edges",
            Type::Face | Type::OnlyFace => "faces",
            Type::EdgeFace => "edges or faces",
        };
        cx.report(
            ReportType::Warning,
            format!("frap: nothing deleted: no mesh in Edit Mode has selected {what}"),
        );
        return Ok(OpStatus::Cancelled);
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}
