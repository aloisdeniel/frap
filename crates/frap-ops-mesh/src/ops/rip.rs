// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.rip(use_fill=False, …)`, tier T1, *headless-extended* (deviation D-150;
//! `tests/golden/cases/edit/tools_m8/rip*.py`). Port of `edbm_rip_invoke` and its helpers
//! (`editors/mesh/editmesh_rip.cc`, [`core`]), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Blender's rip has no `exec`: `invoke` measures every choice in region pixels against the
//! mouse (`event->mval`), so `blender -b` (no window event state) cannot run it. frap runs it
//! as if the mouse were over the **3D cursor** in the factory Layout 3D View
//! ([`view`]): put `scene.cursor.location` on the side to rip towards, then call
//! `mesh.rip` or `mesh.rip_move`. The transform properties (proportional editing, `mirror`,
//! `release_confirm`, `use_accurate`) are only handed to the translation of `rip_move`.
//!
//! Per mesh in Edit Mode with a selected vertex (multi-object edit):
//! - **one vertex** selected (no edge or face): it is split off along the edge nearest to
//!   the mouse (`BM_face_loop_separate_multi`), or the fan nearest to the mouse is split away
//!   (non-manifold vertex, more than two boundary edges, a wire vertex); the part nearest to
//!   the mouse stays selected;
//! - **edges** selected: the selected edges are split (`BM_mesh_edgesplit`, extended by one
//!   edge at the ends of a selection), and of each contiguous run of edges the side facing
//!   away from the mouse is deselected.
//! With `use_fill` the opened gap is closed with new faces (triangles and quads copying the
//! neighbouring corners). Custom normals are kept as vectors and re-encoded in the new corner
//! spaces. Errors (`CANCELLED`): `Cannot rip faces` (faces selected), `Cannot rip multiple
//! disconnected vertices` (several vertices without edges), `Cannot rip non-manifold vertices
//! or edges` (nothing ripped); nothing selected cancels without a report.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

pub(crate) mod core;
pub(crate) mod kernel;
pub(crate) mod view;

use core::{ClnorKeep, RipView};
use view::{Projection, mouse_from_cursor};

#[operator(poll = poll::EDITMESH_VIEW3D, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Rip) -> OpResult {
    let do_fill = p.use_fill.value;
    let (viewmat, cursor) = {
        let c = cx.ctx();
        (
            c.win.view3d.view_matrix.to_cols_array_2d(),
            c.scene().cursor.location,
        )
    };
    let fmval = mouse_from_cursor(&viewmat, cursor);

    let mut no_vertex_selected = true;
    let mut error_face_selected = true;
    let mut error_disconnected_vertices = true;
    let mut error_rip_failed = true;

    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let Some((totvertsel, totedgesel, totfacesel)) = edbm::read(cx, me, edbm::sel_counts)
        else {
            continue;
        };
        if totvertsel == 0 {
            continue;
        }
        no_vertex_selected = false;
        // Running in face mode hardly makes sense (Blender would convert to a region loop).
        if totfacesel > 0 {
            continue;
        }
        error_face_selected = false;
        // Not supported for now.
        if totvertsel > 1 && totedgesel == 0 {
            continue;
        }
        error_disconnected_vertices = false;
        let singlesel = totvertsel == 1 && totedgesel == 0 && totfacesel == 0;
        let obmat = world_matrix_in(cx.main(), ob).to_cols_array_2d();
        let rv = RipView {
            proj: Projection::for_object(&viewmat, &obmat),
            fmval,
        };
        let bm = cx.bmesh_mut(me)?;
        let mut normals = ClnorKeep::to_vector_layer(bm);
        // `BM_ELEM_SELECT` --> `BM_ELEM_TAG` on edges.
        for e in bm.edges.slots.iter_mut().flatten() {
            let s = e.flag.contains(frap_bmesh::ElemFlag::SELECT);
            e.flag.set(frap_bmesh::ElemFlag::TAG, s);
        }
        let finished = if singlesel {
            core::rip_vert(bm, &rv, do_fill, &mut normals)
        } else {
            core::rip_edge(bm, &rv, do_fill, &mut normals)
        };
        if !finished {
            continue;
        }
        normals.from_vector_layer(bm);
        if bm.selected_counts().0 == 0 {
            continue;
        }
        error_rip_failed = false;
        bm.normals_update();
    }

    if no_vertex_selected {
        return Ok(OpStatus::Cancelled);
    }
    let error = if error_face_selected {
        "Cannot rip faces"
    } else if error_disconnected_vertices {
        "Cannot rip multiple disconnected vertices"
    } else if error_rip_failed {
        "Cannot rip non-manifold vertices or edges"
    } else {
        return Ok(OpStatus::Finished);
    };
    cx.report(ReportType::Error, error);
    Ok(OpStatus::Cancelled)
}
