// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.edge_split(type='EDGE')`, tier T1 (`tests/golden/cases/edit/misc/
//! edge_split_*.py`). Port of `edbm_edge_split_exec`, `edbm_edge_split_selected_edges` and
//! `edbm_edge_split_selected_verts` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode (multi-object edit):
//! - `EDGE` (needs a selected edge): `bmesh.ops.split_edges` on the selected edges; every
//!   element is deselected (history cleared), the split edges are selected and the selection
//!   is flushed from the vertices. A single edge of a closed cube gives no new vertex: its
//!   vertices are still fan-connected through the other faces ([MESH §4.11]).
//! - `VERT` (needs a selected vertex): every visible edge with a face and a selected vertex
//!   is split, along with the selected vertices (`use_verts`); the face corners of selected
//!   vertices are remembered on the loops before and their (possibly new) vertices selected
//!   after; selected vertices of wire edges are separated from the other edges
//!   (`BM_vert_separate`); then the selection is flushed from the vertices.

use frap_bmesh::ops::split_edges::{SplitEdgesIn, split_edges};
use frap_bmesh::{BMElem, BMesh, BmError, EdgeId, ElemFlag, LoopId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, edge_split::Type};

use super::rip::core::ClnorKeep;
use super::split::edbm;

/// `edbm_edge_split_selected_verts`, after the selection check (`Err` is the bmesh
/// operator's error, reported by the caller).
fn split_selected_verts(bm: &mut BMesh) -> Result<(), BmError> {
    // `BM_custom_loop_normals_to_vector_layer`: with custom normals, `BM_lnorspace_update`
    // rebuilds the corner spaces when they are dirty, which leaves `BM_ELEM_TAG` on every
    // vertex of a smooth fan. `split_edges` does not clear vertex tags, so those vertices
    // are split too where a tagged edge reaches them (Blender 5.2.4: the smooth cube with
    // custom normals after `extrude_faces_indiv` gives 96 vertices, 80 without them).
    let normals = ClnorKeep::to_vector_layer(bm);
    // Flush from vertices to edges; store the selection in loop tags.
    let mut tagged_edges: Vec<EdgeId> = Vec::new();
    for e in bm.edge_ids() {
        bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        if bm.edge(e).l.is_some() {
            let ed = bm.edge(e);
            let [a, b] = ed.v;
            if !ed.flag.contains(ElemFlag::HIDDEN)
                && (bm.vert(a).flag.contains(ElemFlag::SELECT)
                    || bm.vert(b).flag.contains(ElemFlag::SELECT))
            {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
                tagged_edges.push(e);
            }
            for l in bm.edge_loops(e) {
                let sel = bm.vert(bm.lp(l).v).flag.contains(ElemFlag::SELECT);
                bm.lp_mut(l).flag.set(ElemFlag::TAG, sel);
            }
        }
    }
    let verts = edbm::selected_verts(bm);
    let r = split_edges(
        bm,
        &SplitEdgesIn {
            edges: tagged_edges,
            verts,
            use_verts: true,
        },
    );
    r?;
    for e in bm.edge_ids() {
        if !bm.edges.contains(e.0) {
            continue;
        }
        if bm.edge(e).l.is_some() {
            let loops: Vec<LoopId> = bm.edge_loops(e);
            for l in loops {
                if bm.lp(l).flag.contains(ElemFlag::TAG) {
                    let v = bm.lp(l).v;
                    bm.vert_select_set(v, true);
                }
            }
        } else {
            // Split out wire.
            for i in 0..2 {
                let v = bm.edge(e).v[i];
                if bm.vert(v).flag.contains(ElemFlag::SELECT) && bm.disk_edge_next(e, v) != e {
                    bm.vert_separate(v, &[e], true);
                }
            }
        }
    }
    // `BM_custom_loop_normals_from_vector_layer(bm, false)`: the corners keep their slots
    // (separating a vertex moves loops, it does not make new ones).
    normals.restore(bm);
    bm.select_flush();
    Ok(())
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::EdgeSplit) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let (v, e, _) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        match p.r#type.value {
            Type::Vert => {
                if v == 0 {
                    continue;
                }
                let r = split_selected_verts(cx.bmesh_mut(me)?);
                edbm::finish(cx, r)?;
            }
            Type::Edge => {
                if e == 0 {
                    continue;
                }
                let bm = cx.bmesh_mut(me)?;
                let edges = edbm::selected_edges(bm);
                let r = split_edges(
                    bm,
                    &SplitEdgesIn {
                        edges,
                        ..SplitEdgesIn::default()
                    },
                );
                let Some(out) = edbm::finish(cx, r)? else {
                    continue;
                };
                let bm = cx.bmesh_mut(me)?;
                // `EDBM_op_call_and_selectf(…, "edges.out", false, …)`.
                edbm::deselect_all(bm);
                let elems: Vec<BMElem> = out.edges.iter().map(|&e| BMElem::Edge(e)).collect();
                edbm::select_elems(bm, &elems);
                bm.select_flush();
            }
        }
    }
    Ok(OpStatus::Finished)
}
