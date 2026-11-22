// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_linked_pick(deselect=False, delimit={'SEAM'}, object_index=-1,
//! index=-1)`, tier T1 (`tests/golden/cases/edit/select_m8/linked_pick.py`). Port of
//! `edbm_select_linked_pick_exec`, `edbm_select_linked_pick_ex` (with
//! `select_linked_delimit_validate/begin`) and `EDBM_elem_from_index_any_multi`
//! (`editors/mesh/editmesh_select.cc`, `editmesh_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! *Headless-extended* as WP 2.4's pick operators: Blender's exec (the redo path of the mouse
//! pick) selects by index, which is how agents use it. `object_index` is the position of the
//! object among the objects in Edit Mode (active first), `index` the element: a vertex index,
//! else `index - len(vertices)` an edge, else `index - len(vertices) - len(edges)` a face.
//! Without either, or out of range, the operator is `CANCELLED`, as in Blender.
//!
//! From that element everything connected is selected (or deselected with `deselect`) without
//! crossing the `delimit` edges: a vertex or an edge walks connected edges (`BMW_VERT_SHELL`),
//! or with delimiters face corners and wire edges (`BMW_LOOP_SHELL_WIRE`), then the selection
//! is flushed; a face walks its face island (`BMW_ISLAND`). An unset `delimit` is Blender's
//! initial default for the edit mesh's select mode (none in vertex/edge mode, `{'SEAM'}` in
//! face-only mode; Blender then remembers the last `delimit` given to `select_linked` or
//! `select_linked_pick` in the session, frap does not).

use frap_bmesh::walkers::WalkDelimit;
use frap_bmesh::{BMElem, BMesh, EdgeId, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::MeshId;
use frap_spec_ops_mesh::mesh;

use super::loop_select::walk::{Island, LoopShellWire, ShellItem, Start, VertShell};
use super::select_linked::delimit_from;
use super::select_more::common;

/// `EDBM_elem_from_index_any`.
pub(crate) fn elem_from_index_any(bm: &BMesh, index: usize) -> Option<BMElem> {
    let verts = bm.vert_ids();
    if index < verts.len() {
        return Some(BMElem::Vert(verts[index]));
    }
    let index = index - verts.len();
    let edges = bm.edge_ids();
    if index < edges.len() {
        return Some(BMElem::Edge(edges[index]));
    }
    let index = index - edges.len();
    bm.face_ids().get(index).copied().map(BMElem::Face)
}

/// `edbm_select_linked_pick_ex`.
fn select_linked_pick(bm: &mut BMesh, ele: BMElem, select: bool, delimit: WalkDelimit) {
    let mut delimit = delimit;
    // `select_linked_delimit_validate`.
    if delimit.uv && bm.ldata.float2_names().is_empty() {
        delimit.uv = false;
    }
    let any = delimit != WalkDelimit::default();
    // `BMO_ELE_TAG` of `select_linked_delimit_begin`: the edges a walk may cross.
    let walk_ok: Vec<bool> = {
        let mut ok = vec![true; bm.edges.capacity()];
        if any {
            for e in bm.edge_ids() {
                ok[e.slot()] = !bm.edge_delimits(e, delimit);
            }
        }
        ok
    };
    let mask = |e: EdgeId| walk_ok.get(e.slot()).copied().unwrap_or(true);
    match ele {
        BMElem::Vert(_) | BMElem::Edge(_) => {
            let start = match ele {
                BMElem::Vert(v) => Start::Vert(v),
                BMElem::Edge(e) => Start::Edge(e),
                BMElem::Face(_) => unreachable!("vertex or edge"),
            };
            if any {
                let items = LoopShellWire::new(&mask).walk(bm, start);
                for item in items {
                    match (item, ele) {
                        (ShellItem::Loop(l), BMElem::Vert(_)) => {
                            let v = bm.lp(l).v;
                            bm.vert_select_set(v, select);
                        }
                        (ShellItem::Loop(l), _) => {
                            let e = bm.lp(l).e;
                            bm.edge_select_set(e, select);
                        }
                        (ShellItem::Edge(e), _) => bm.edge_select_set(e, select),
                    }
                }
            } else {
                for e in VertShell::new().walk(bm, start) {
                    bm.edge_select_set(e, select);
                }
            }
            common::selectmode_flush(bm);
        }
        BMElem::Face(f) => {
            for x in Island::new(&mask).walk(bm, f) {
                bm.face_select_set(x, select);
            }
        }
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "select by index like Blender's redo path: object_index (objects in Edit Mode, active first) and index (vertices, then edges, then faces) are required; without them the operator is CANCELLED",
        "an unset delimit is Blender's initial default for the select mode: none in vertex/edge mode, {'SEAM'} in face-only mode (Blender then remembers the last delimit passed in the session; frap does not)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::SelectLinkedPick) -> OpResult {
    // Intentionally wrap negative values so the lookup fails (Blender's `uint` casts).
    let object_index = p.object_index.value as u32 as usize;
    let index = p.index.value as u32 as usize;
    let bases: Vec<frap_rna::IdRef<MeshId>> = {
        let c = cx.ctx();
        c.objects_in_mode()
            .into_iter()
            .filter_map(|r| c.main.objects.get(r)?.data?.typed::<MeshId>())
            .collect()
    };
    let Some(&me) = bases.get(object_index) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(Some((ele, mode))) =
        common::read(cx, me, |bm| Some((elem_from_index_any(bm, index)?, bm.select_mode)))
    else {
        return Ok(OpStatus::Cancelled);
    };
    let delimit = if p.delimit.is_set {
        delimit_from(&p.delimit.value)
    } else {
        // `delimit_last_store[(selectmode & (VERTEX | EDGE)) == 0]` = {0, SEAM}.
        WalkDelimit {
            seam: !mode.intersects(SelectMode::VERT | SelectMode::EDGE),
            ..WalkDelimit::default()
        }
    };
    let bm = cx.bmesh_mut(me)?;
    select_linked_pick(bm, ele, !p.deselect.value, delimit);
    Ok(OpStatus::Finished)
}
