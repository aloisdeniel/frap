// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.shortest_path_pick(edge_mode='SELECT', use_face_step=False,
//! use_topology_distance=False, use_fill=False, skip=0, nth=1, offset=0, index=-1)`, tier T1
//! (`tests/golden/cases/edit/select_m8/path_pick.py`). Port of `edbm_shortest_path_pick_exec`,
//! `edbm_shortest_path_pick_ex`, `edbm_elem_active_elem_or_face_get`,
//! `EDBM_elem_from_index_any` (`editors/mesh/editmesh_path.cc`, `editmesh_utils.cc`) and
//! `material_active_index_set` (`editors/object/object_utils.cc`), Blender 5.2.4,
//! GPL-2.0-or-later; the path selection is `shortest_path_select`'s with `track_active`.
//!
//! *Headless-extended* as WP 2.4's pick operators: Blender's exec (the redo path of the mouse
//! pick) takes the picked element by `index` (a vertex index, else `index - len(vertices)` an
//! edge, else `index - len(vertices) - len(edges)` a face of the context edit mesh). The path
//! runs from the active element (the last selection-history entry, else the selected active
//! face) to the picked element of the same type and is selected, or deselected when already
//! fully selected; the end of the path becomes the active element (a face also sets the
//! object's active material index). In edge mode `edge_mode` may tag seams, sharp edges,
//! creases, bevel weights or Freestyle marks instead. Without `index`, an `index` out of range,
//! no active element or a picked element of another type the operator is `CANCELLED`, as in
//! Blender. An unset `edge_mode` is the tool setting's factory value `SELECT`.

use frap_bmesh::{BMElem, BMesh};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::MeshId;
use frap_spec_ops_mesh::mesh::{self, shortest_path_pick, shortest_path_select::EdgeMode};

use super::select_linked_pick::elem_from_index_any;
use super::select_more::common;
use super::shortest_path_select::path::PathParams;
use super::shortest_path_select::{
    Interval, Params, shortest_path_edge, shortest_path_face, shortest_path_vert,
};

/// `edbm_elem_active_elem_or_face_get`.
fn active_elem_or_face(bm: &BMesh) -> Option<BMElem> {
    if let Some(&e) = bm.select_history.last() {
        return Some(e);
    }
    bm.act_face
        .filter(|&f| bm.elem_alive(BMElem::Face(f)) && common::selected(bm, f))
        .map(BMElem::Face)
}

fn edge_mode(m: shortest_path_pick::EdgeMode) -> EdgeMode {
    match m {
        shortest_path_pick::EdgeMode::Select => EdgeMode::Select,
        shortest_path_pick::EdgeMode::Seam => EdgeMode::Seam,
        shortest_path_pick::EdgeMode::Sharp => EdgeMode::Sharp,
        shortest_path_pick::EdgeMode::Crease => EdgeMode::Crease,
        shortest_path_pick::EdgeMode::Bevel => EdgeMode::Bevel,
        shortest_path_pick::EdgeMode::Freestyle => EdgeMode::Freestyle,
    }
}

#[operator(
    poll = poll::EDITMESH_VIEW3D,
    tier = T1,
    notes = [
        "the picked element is given by index (vertices, then edges, then faces of the edit mesh) like Blender's redo path; without it the operator is CANCELLED",
        "an unset edge_mode is the factory tool setting SELECT (frap keeps no tool_settings.edge_mode)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::ShortestPathPick) -> OpResult {
    let edit = {
        let c = cx.ctx();
        poll::edit_object(&c).and_then(|(r, o)| Some((r, o.data?.typed::<MeshId>()?)))
    };
    let Some((obedit, me)) = edit else {
        return Ok(OpStatus::Cancelled);
    };
    let index = p.index.value;
    let ends = common::read(cx, me, |bm| {
        let total = bm.verts_num() + bm.edges_num() + bm.faces_num();
        if index < 0 || index as usize >= total {
            return None;
        }
        let src = active_elem_or_face(bm)?;
        let dst = elem_from_index_any(bm, index as usize)?;
        Some((src, dst))
    })
    .flatten();
    let Some((src, dst)) = ends else {
        return Ok(OpStatus::Cancelled);
    };
    let params = Params {
        edge_mode: edge_mode(p.edge_mode.value),
        path: PathParams {
            use_topology_distance: p.use_topology_distance.value,
            use_step_face: p.use_face_step.value,
        },
        use_fill: p.use_fill.value,
        interval: Interval::new(p.nth.value, p.skip.value, p.offset.value),
        track_active: true,
    };
    let bm = cx.bmesh_mut(me)?;
    let face_last = match (src, dst) {
        (BMElem::Vert(a), BMElem::Vert(b)) => {
            shortest_path_vert(bm, &params, a, b);
            None
        }
        (BMElem::Edge(a), BMElem::Edge(b)) => {
            shortest_path_edge(bm, &params, a, b);
            None
        }
        (BMElem::Face(a), BMElem::Face(b)) => {
            let f = shortest_path_face(bm, &params, a, b);
            Some(i32::from(bm.face(f).mat_nr))
        }
        _ => return Ok(OpStatus::Cancelled),
    };
    if let Some(mat_nr) = face_last {
        // `material_active_index_set(obedit, f_dst_last->mat_nr)`.
        let totcol = {
            let main = cx.main();
            main.objects
                .get(obedit)
                .map_or(0, |o| frap_scene::id::slots::slot_count(main, o))
        };
        if totcol > 0 {
            let actcol = (mat_nr + 1).clamp(1, totcol as i32);
            let (main, log) = cx.main_mut();
            if let Some(o) = main.objects.get_mut(obedit, log)
                && o.active_material_index != actcol - 1
            {
                o.active_material_index = actcol - 1;
            }
        }
    }
    Ok(OpStatus::Finished)
}
