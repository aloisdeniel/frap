// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.delete_loose(use_verts=True, use_edges=True, use_faces=False)`, tier T1
//! (`tests/golden/cases/edit/delete/delete_loose.py`). Port of `edbm_delete_loose_exec` and
//! `bm_face_is_loose` (`editors/mesh/editmesh_tools.cc`) with `BM_mesh_delete_hflag_context`
//! (`bmesh/intern/bmesh_delete.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Each kind is used only when some edit mesh has that kind selected. On every mesh in Edit
//! Mode: selected faces whose edges are all boundary edges are deleted (`FACES` context, so
//! their edges and vertices go too), then selected wire edges (`EDGES`), then selected
//! vertices without edges; then everything is deselected. Always `FINISHED`, with the INFO
//! report `Removed: N vertices, N edges, N faces` (all edit meshes together).

use frap_bmesh::{BMElem, BMesh, ElemFlag};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, totals};

/// `bm_face_is_loose`: every edge of the face is a boundary edge.
fn face_is_loose(bm: &BMesh, f: frap_bmesh::FaceId) -> bool {
    bm.face_edges(f).into_iter().all(|e| bm.edge_is_boundary(e))
}

/// `BM_mesh_delete_hflag_context(bm, BM_ELEM_TAG, context)` for the contexts used here.
fn delete_tagged(bm: &mut BMesh, context: &str) {
    let tagged = |f: ElemFlag| f.contains(ElemFlag::TAG);
    let geom: Vec<BMElem> = match context {
        "FACES" => bm
            .faces
            .iter()
            .filter(|(_, f)| tagged(f.flag))
            .map(|(i, _)| BMElem::Face(frap_bmesh::FaceId(i)))
            .collect(),
        "EDGES" => {
            // The tag is flushed down to the edges' vertices (and stays on survivors).
            let edges: Vec<_> = bm
                .edges
                .iter()
                .filter(|(_, e)| tagged(e.flag))
                .map(|(i, _)| frap_bmesh::EdgeId(i))
                .collect();
            for &e in &edges {
                for v in bm.edge(e).v {
                    bm.vert_mut(v).flag.insert(ElemFlag::TAG);
                }
            }
            edges.into_iter().map(BMElem::Edge).collect()
        }
        _ => bm
            .verts
            .iter()
            .filter(|(_, v)| tagged(v.flag))
            .map(|(i, _)| BMElem::Vert(frap_bmesh::VertId(i)))
            .collect(),
    };
    bm.delete_context(&geom, context);
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "each kind is used only when some edit mesh has that kind selected",
        "always FINISHED with the INFO report 'Removed: N vertices, N edges, N faces'; result.removed / removed_edges / removed_faces",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::DeleteLoose) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let mut sel = [0usize; 3];
    for &(_, me) in &meshes {
        if let Some(bm) = super::delete::common::edit_bm(cx, me) {
            let (v, e, f) = bm.selected_counts();
            sel[0] += v;
            sel[1] += e;
            sel[2] += f;
        }
    }
    let use_verts = p.use_verts.value && sel[0] > 0;
    let use_edges = p.use_edges.value && sel[1] > 0;
    let use_faces = p.use_faces.value && sel[2] > 0;
    let mut removed = Removed::default();
    for (_, me) in meshes {
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        for f in bm.verts.slots.iter_mut().flatten() {
            f.flag.remove(ElemFlag::TAG);
        }
        for e in bm.edges.slots.iter_mut().flatten() {
            e.flag.remove(ElemFlag::TAG);
        }
        for f in bm.faces.slots.iter_mut().flatten() {
            f.flag.remove(ElemFlag::TAG);
        }
        if use_faces {
            for f in bm.face_ids() {
                if bm.face(f).flag.contains(ElemFlag::SELECT) {
                    let loose = face_is_loose(bm, f);
                    bm.face_mut(f).flag.set(ElemFlag::TAG, loose);
                }
            }
            delete_tagged(bm, "FACES");
        }
        if use_edges {
            for e in bm.edge_ids() {
                if bm.edge(e).flag.contains(ElemFlag::SELECT) {
                    let wire = bm.edge_is_wire(e);
                    bm.edge_mut(e).flag.set(ElemFlag::TAG, wire);
                }
            }
            delete_tagged(bm, "EDGES");
        }
        if use_verts {
            for v in bm.vert_ids() {
                if bm.vert(v).flag.contains(ElemFlag::SELECT) {
                    let loose = bm.vert(v).e.is_none();
                    bm.vert_mut(v).flag.set(ElemFlag::TAG, loose);
                }
            }
            delete_tagged(bm, "VERTS");
        }
        bm.deselect_all();
        removed.add(old, totals(bm));
    }
    cx.report(ReportType::Info, removed.delete_info());
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}
