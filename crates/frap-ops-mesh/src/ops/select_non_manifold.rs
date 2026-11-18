// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_non_manifold(extend=True, use_wire=True, use_boundary=True,
//! use_multi_face=True, use_non_contiguous=True, use_verts=True)`, tier T1
//! (`tests/golden/cases/edit/select/non_manifold*.py`). Port of
//! `edbm_select_non_manifold_exec`, `edbm_vert_or_edge_select_mode_poll` and
//! `edbm_selectmode_sync_multi_ex` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Only in vertex or edge select mode (the poll fails in face-only mode with Blender's message
//! `An edit-mesh with vertex or edge selection mode is required`). Every other mesh in Edit
//! Mode first takes the active mesh's select mode. Then, per mesh (after deselecting all
//! unless `extend`): visible non-manifold vertices (`use_verts`) and visible edges that are
//! wire, boundary, between faces of opposite winding (`use_non_contiguous`) or shared by more
//! than two faces (`use_multi_face`) are selected, and the selection is flushed. On the grid
//! primitive this selects the border vertices and edges.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_mesh::mesh;
use smallvec::smallvec;

use super::select_more::common;

const VERT_OR_EDGE_MSG: &str = "An edit-mesh with vertex or edge selection mode is required";

/// `edbm_vert_or_edge_select_mode_poll` (frap reads the select mode from
/// `tool_settings.mesh_select_mode`, which every edit mesh follows).
pub(crate) fn vert_or_edge_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
    let message = Some(format!(
        "Operator bpy.ops.{idname}.poll() {VERT_OR_EDGE_MSG}"
    ));
    if let Err(mut f) = poll::editmesh(c) {
        f.message = message;
        return Err(f);
    }
    let m = c.scene().tool_settings.mesh_select_mode;
    if m[0] || m[1] {
        return Ok(());
    }
    Err(PollFail {
        message,
        unmet: smallvec![],
        hint: Some("bpy.ops.mesh.select_mode(type='VERT')".into()),
    })
}

fn check(c: &Context) -> Result<(), PollFail> {
    vert_or_edge_poll(c, "mesh.select_non_manifold")
}

/// `edbm_vert_or_edge_select_mode_poll`.
const POLL: Poll = Poll {
    requires: &[PollReq::ModeEditMesh],
    check,
};

#[operator(
    poll = POLL,
    tier = T1,
    notes = ["vertex or edge select mode only (Blender's poll fails in face-only mode)"]
)]
fn exec(cx: &mut OpCtx, p: mesh::SelectNonManifold) -> OpResult {
    let meshes = common::edit_meshes(cx);
    // `edbm_selectmode_sync_multi_ex`: every mesh takes the active mesh's select mode.
    if meshes.len() > 1 {
        let active = common::read(cx, meshes[0].1, |bm| bm.select_mode);
        if let Some(mode) = active {
            for &(_, me) in &meshes[1..] {
                if common::read(cx, me, |bm| bm.select_mode) != Some(mode) {
                    cx.bmesh_mut(me)?.selectmode_set(mode);
                }
            }
        }
    }
    for (_, me) in meshes {
        let bm = cx.bmesh_mut(me)?;
        let mut changed = false;
        if !p.extend.value {
            common::deselect_all(bm);
            changed = true;
        }
        // Isolated vertices, and edges that do not have two neighbouring faces.
        if p.use_verts.value {
            for v in bm.vert_ids() {
                if common::hidden(bm, v) {
                    continue;
                }
                if !bm.vert_is_manifold(v) {
                    bm.vert_select_set(v, true);
                    changed = true;
                }
            }
        }
        let (wire, boundary, multi, non_contiguous) = (
            p.use_wire.value,
            p.use_boundary.value,
            p.use_multi_face.value,
            p.use_non_contiguous.value,
        );
        if wire || boundary || multi || non_contiguous {
            for e in bm.edge_ids() {
                if common::hidden(bm, e) {
                    continue;
                }
                if (wire && bm.edge_is_wire(e))
                    || (boundary && bm.edge_is_boundary(e))
                    || (non_contiguous && bm.edge_is_manifold(e) && !bm.edge_is_contiguous(e))
                    || (multi && bm.edge_face_count(e) > 2)
                {
                    bm.edge_select_set(e, true);
                    changed = true;
                }
            }
        }
        if changed {
            common::selectmode_flush(bm);
        }
    }
    Ok(OpStatus::Finished)
}
