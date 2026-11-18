// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.dissolve_limited(angle_limit=5°, use_dissolve_boundaries=False,
//! delimit={'NORMAL'})`, tier T1 (`tests/golden/cases/edit/delete/dissolve_limited*.py`).
//! Port of `edbm_dissolve_limited_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode with anything selected, `bmesh.ops.dissolve_limit` runs on the
//! selected edges and vertices (in face-only select mode: the edges and vertices of selected
//! faces that no unselected face uses), merging faces flatter than `angle_limit` (capped at
//! 90°) and dissolving vertices between nearly straight edges; the merged faces are selected
//! (the selection is extended). Always `FINISHED`.

use frap_bmesh::ops::dissolve_limit::{DissolveLimitIn, dissolve_limit};
use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, sel_edges, sel_verts, select_elems, totals};

/// `delimit` identifiers as `'static` strings (`rna_enum_mesh_delimit_mode_items`).
fn delimit_static(s: &str) -> Option<&'static str> {
    ["NORMAL", "MATERIAL", "SEAM", "SHARP", "UV"]
        .into_iter()
        .find(|x| *x == s)
}

/// Face select mode: the vertices and edges of selected faces, minus those an unselected face
/// uses (flushed to `BM_ELEM_TAG`).
fn face_mode_input(bm: &mut BMesh) -> (Vec<VertId>, Vec<EdgeId>) {
    for v in bm.verts.slots.iter_mut().flatten() {
        let s = v.flag.contains(ElemFlag::SELECT);
        v.flag.set(ElemFlag::TAG, s);
    }
    for e in bm.edges.slots.iter_mut().flatten() {
        let s = e.flag.contains(ElemFlag::SELECT);
        e.flag.set(ElemFlag::TAG, s);
    }
    for f in bm.face_ids() {
        if !bm.face(f).flag.contains(ElemFlag::SELECT) {
            for l in bm.face_loops(f) {
                let (v, e) = (bm.lp(l).v, bm.lp(l).e);
                bm.vert_mut(v).flag.remove(ElemFlag::TAG);
                bm.edge_mut(e).flag.remove(ElemFlag::TAG);
            }
        }
    }
    let tagged = |f: ElemFlag| f.contains(ElemFlag::TAG) && !f.contains(ElemFlag::HIDDEN);
    let verts = bm
        .verts
        .iter()
        .filter(|(_, v)| tagged(v.flag))
        .map(|(i, _)| VertId(i))
        .collect();
    let edges = bm
        .edges
        .iter()
        .filter(|(_, e)| tagged(e.flag))
        .map(|(i, _)| EdgeId(i))
        .collect();
    (verts, edges)
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "angle_limit is in radians (default 5 degrees = 0.0873) and is capped at 90 degrees",
        "the merged faces are selected; faces and vertices are dissolved cheapest first in Blender's heap order",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::DissolveLimited) -> OpResult {
    let delimit: Vec<&'static str> = p
        .delimit
        .value
        .iter()
        .filter_map(|s| delimit_static(s))
        .collect();
    let mut removed = Removed::default();
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts() == (0, 0, 0)) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        let (verts, edges) = if bm.select_mode == SelectMode::FACE {
            face_mode_input(bm)
        } else {
            (sel_verts(bm), sel_edges(bm))
        };
        let i = DissolveLimitIn {
            angle_limit: p.angle_limit.value,
            use_dissolve_boundaries: p.use_dissolve_boundaries.value,
            verts,
            edges,
            delimit: delimit.clone(),
        };
        let out = dissolve_limit(bm, &i).map_err(bm_err)?;
        let region: Vec<BMElem> = out.region.iter().map(|&f| BMElem::Face(f)).collect();
        select_elems(bm, &region);
        removed.add(old, totals(bm));
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}
