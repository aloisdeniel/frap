// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.rip(use_fill=False, …, frap_location=…, frap_direction=…)`, tier T1,
//! *headless-extended* (deviation D-150; `tests/golden/cases/edit/tools_m8/rip*.py`, the
//! interactive recordings of `tests/golden/cases/edit/tools_m8/rip_recordings/`). Port of
//! `edbm_rip_invoke` and its helpers (`editors/mesh/editmesh_rip.cc`, [`core`]), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Blender's rip has no `exec`: `invoke` measures every choice in region pixels against the
//! mouse (`event->mval`), so `blender -b` (no window event state) cannot run it. frap runs it
//! with the mouse over a world-space point ([`view`]):
//! - `frap_location`, or the **3D cursor** (`scene.cursor.location`) when it is not set: the
//!   side to rip towards;
//! - seen in the factory Layout 3D View, or, with a non-zero `frap_direction`, in an
//!   orthographic view looking along `frap_direction` (the mouse ray is then
//!   `frap_location` + t·`frap_direction`, as `mesh.rip_edge`'s `location`/`direction`), so
//!   that geometry the factory view sees edge-on or from behind can still be ripped towards a
//!   stated side.
//!
//! The transform properties (proportional editing, `mirror`, `release_confirm`,
//! `use_accurate`) are only handed to the translation of `rip_move`.
//!
//! Per mesh in Edit Mode with a selected vertex (multi-object edit):
//! - **one vertex** selected (no edge or face): it is split off along the edge nearest to
//!   the mouse (`BM_face_loop_separate_multi`), or the fan nearest to the mouse is split away
//!   (non-manifold vertex, more than two boundary edges, a wire vertex); the part nearest to
//!   the mouse stays selected;
//! - **edges** selected: the selected edges are split (`BM_mesh_edgesplit`, extended by one
//!   edge at the ends of a selection), and of each contiguous run of edges the side facing
//!   away from the mouse is deselected.
//!
//! With `use_fill` the opened gap is closed with new faces (triangles and quads copying the
//! neighbouring corners). Custom normals go through a vector layer and are re-encoded in the
//! new corner spaces, averaged per smooth fan, as Blender does. Errors (`RuntimeError`, as
//! Blender's ERROR report raises): `Cannot rip faces` (faces selected), `Cannot rip multiple
//! disconnected vertices` (several vertices without edges), `Cannot rip non-manifold vertices
//! or edges` (nothing ripped); nothing selected cancels without a report.

use frap_bmesh::BMesh;
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

pub(crate) mod core;
pub(crate) mod fan_spaces;
pub(crate) mod kernel;
pub(crate) mod view;

#[cfg(test)]
mod tests;

use self::core::{ClnorKeep, RipView};
use view::{Projection, RegionView};

/// What the rip did to one edit mesh (the error bookkeeping of `edbm_rip_invoke`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MeshRip {
    /// No vertex selected: skipped.
    NoVertexSelected,
    /// Faces selected.
    FaceSelected,
    /// Several vertices and no edge selected.
    DisconnectedVertices,
    /// The rip path cancelled (nothing to rip).
    Cancelled,
    /// Ripped, but nothing is left selected.
    NothingSelected,
    /// Ripped.
    Ripped,
}

/// The selection checks of `edbm_rip_invoke` that skip a mesh before anything is changed.
pub(crate) fn precheck(bm: &BMesh) -> Option<MeshRip> {
    let (totvertsel, totedgesel, totfacesel) = edbm::sel_counts(bm);
    if totvertsel == 0 {
        return Some(MeshRip::NoVertexSelected);
    }
    // Running in face mode hardly makes sense (Blender would convert to a region loop).
    if totfacesel > 0 {
        return Some(MeshRip::FaceSelected);
    }
    // Not supported for now.
    if totvertsel > 1 && totedgesel == 0 {
        return Some(MeshRip::DisconnectedVertices);
    }
    None
}

/// The body of `edbm_rip_invoke`'s loop for one mesh: `rv` is the region the mouse `fmval` is
/// in, `obmat` the object's world matrix.
pub(crate) fn rip_mesh(
    bm: &mut BMesh,
    rv: &RegionView,
    obmat: &[[f32; 4]; 4],
    fmval: [f32; 2],
    do_fill: bool,
) -> MeshRip {
    if let Some(skip) = precheck(bm) {
        return skip;
    }
    let (totvertsel, totedgesel, totfacesel) = edbm::sel_counts(bm);
    let singlesel = totvertsel == 1 && totedgesel == 0 && totfacesel == 0;
    let rv = RipView {
        proj: Projection::for_object(rv, obmat),
        fmval,
    };
    let mut normals = ClnorKeep::to_vector_layer(bm);
    // `BM_ELEM_SELECT` --> `BM_ELEM_TAG` on edges.
    for e in bm.edges.slots.iter_mut().flatten() {
        let s = e.flag.contains(frap_bmesh::ElemFlag::SELECT);
        e.flag.set(frap_bmesh::ElemFlag::TAG, s);
    }
    let finished = if singlesel {
        self::core::rip_vert(bm, &rv, do_fill, &mut normals)
    } else {
        self::core::rip_edge(bm, &rv, do_fill, &mut normals)
    };
    if !finished {
        return MeshRip::Cancelled;
    }
    normals.restore(bm);
    if bm.selected_counts().0 == 0 {
        return MeshRip::NothingSelected;
    }
    bm.normals_update();
    MeshRip::Ripped
}

/// The report of `edbm_rip_invoke` for the per-mesh outcomes: `None` cancels without one
/// (nothing selected), `Some(Ok)` is `FINISHED`, `Some(Err(message))` an ERROR report.
pub(crate) fn rip_report(outcomes: &[MeshRip]) -> Option<Result<(), &'static str>> {
    let past = |o: &MeshRip, stage: u8| {
        let s = match o {
            MeshRip::NoVertexSelected => 0,
            MeshRip::FaceSelected => 1,
            MeshRip::DisconnectedVertices => 2,
            MeshRip::Cancelled | MeshRip::NothingSelected => 3,
            MeshRip::Ripped => 4,
        };
        s > stage
    };
    if !outcomes.iter().any(|o| past(o, 0)) {
        return None;
    }
    Some(if !outcomes.iter().any(|o| past(o, 1)) {
        Err("Cannot rip faces")
    } else if !outcomes.iter().any(|o| past(o, 2)) {
        Err("Cannot rip multiple disconnected vertices")
    } else if !outcomes.iter().any(|o| past(o, 3)) {
        Err("Cannot rip non-manifold vertices or edges")
    } else {
        Ok(())
    })
}

#[operator(
    poll = poll::EDITMESH_VIEW3D,
    tier = T1,
    support = HeadlessExtended("headless-extended (deviations D-150): blender -b cannot run mesh.rip (invoke only, no window event); frap runs Blender's algorithm with the mouse over frap_location (unset: the 3D cursor), seen in the factory Layout 3D View (1574x954 WINDOW region) or, with a non-zero frap_direction, in an orthographic view looking along frap_direction: put the point on the side to rip towards")
)]
fn exec(cx: &mut OpCtx, p: mesh::Rip) -> OpResult {
    let do_fill = p.use_fill.value;
    let (viewmat, cursor) = {
        let c = cx.ctx();
        (
            c.win.view3d.view_matrix.to_cols_array_2d(),
            c.scene().cursor.location,
        )
    };
    let point = if p.frap_location.is_set {
        p.frap_location.value
    } else {
        cursor
    };
    let dir = p.frap_direction.value;
    let region = if dir != [0.0; 3] {
        RegionView::ortho_along(dir, point)
    } else {
        RegionView::factory(viewmat)
    };
    let fmval = region.mouse_over(point);

    let mut outcomes = Vec::new();
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        match edbm::read(cx, me, precheck) {
            None => continue,
            Some(Some(skip)) => {
                outcomes.push(skip);
                continue;
            }
            Some(None) => {}
        }
        let obmat = world_matrix_in(cx.main(), ob).to_cols_array_2d();
        let bm = cx.bmesh_mut(me)?;
        outcomes.push(rip_mesh(bm, &region, &obmat, fmval, do_fill));
    }
    match rip_report(&outcomes) {
        // Nothing selected: ignore it.
        None => Ok(OpStatus::Cancelled),
        Some(Ok(())) => Ok(OpStatus::Finished),
        // Blender's ERROR report makes the call raise (nothing observable was changed).
        Some(Err(error)) => Err(OpError::Runtime(error.into())),
    }
}
