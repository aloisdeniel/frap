// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_ungrouped(extend=False)`, tier T1 (`tests/golden/cases/edit/select_m8/
//! ungrouped.py`). Port of `edbm_select_ungrouped_exec` and `edbm_select_ungrouped_poll`
//! (`editors/mesh/editmesh_select.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Poll: a mesh in Edit Mode in vertex select mode (else `Must be in vertex selection mode`)
//! whose object has vertex groups and whose edit mesh has deform weights (else `No
//! weights/vertex groups on object`). Per mesh in Edit Mode with deform weights: unless
//! `extend`, a mesh with selected vertices is deselected; every visible vertex in no group (no
//! weight entry at all, a zero weight still counts as grouped) is selected, then the selection
//! is flushed.
//!
//! frap's polls see the context, not the edit BMesh: the select mode and the vertex groups
//! are checked before the arguments, the edit mesh's deform weights at the start of the
//! execution (same error).

use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq, operator};
use frap_rna::{FrapError, MeshId, PyExc};
use frap_scene::Context;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;

const NO_WEIGHTS: &str = "No weights/vertex groups on object";

fn poll_fail(msg: &str) -> PollFail {
    PollFail {
        message: Some(format!(
            "Operator bpy.ops.mesh.select_ungrouped.poll() {msg}"
        )),
        unmet: Default::default(),
        hint: None,
    }
}

/// The poll's context part: Edit Mode, vertex select mode, vertex groups.
fn poll_ungrouped(c: &Context) -> Result<(), PollFail> {
    poll::editmesh(c)?;
    if !c.tool_settings().mesh_select_mode[0] {
        return Err(poll_fail("Must be in vertex selection mode"));
    }
    let groups = poll::edit_object(c)
        .and_then(|(_, o)| o.data?.typed::<MeshId>())
        .and_then(|me| c.main.meshes.get(me))
        .is_some_and(|m| !m.vertex_group_names.is_empty());
    if !groups {
        return Err(poll_fail(NO_WEIGHTS));
    }
    Ok(())
}

const POLL: Poll = Poll {
    requires: &[PollReq::ModeEditMesh],
    check: poll_ungrouped,
};

#[operator(
    poll = POLL,
    tier = T1,
    notes = [
        "the poll's check of the edit mesh's deform weights runs after the arguments are converted (frap's polls see the context only)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::SelectUngrouped) -> OpResult {
    // The rest of `edbm_select_ungrouped_poll`: the edit object's mesh needs deform weights.
    let active = {
        let c = cx.ctx();
        poll::edit_object(&c).and_then(|(_, o)| o.data?.typed::<MeshId>())
    };
    let has_dvert = active
        .and_then(|me| common::read(cx, me, |bm| bm.vdata.deform.is_some()))
        .unwrap_or(false);
    if !has_dvert {
        let mut e = FrapError::new(
            PyExc::RuntimeError,
            frap_op::errors::OP_POLL_FAILED,
            format!("Operator bpy.ops.mesh.select_ungrouped.poll() {NO_WEIGHTS}"),
        );
        e.hint = None;
        return Err(OpError::Frap(Box::new(e)));
    }
    for (_, me) in common::edit_meshes(cx) {
        let Some(Some(ungrouped)) = common::read(cx, me, |bm| {
            let dv = bm.vdata.deform.as_ref()?;
            Some(
                bm.vert_ids()
                    .into_iter()
                    .filter(|&v| {
                        !common::hidden(bm, v)
                            && dv.get(v.slot()).is_none_or(|d| d.weights.is_empty())
                    })
                    .collect::<Vec<_>>(),
            )
        }) else {
            continue;
        };
        let any_selected = common::read(cx, me, |bm| bm.selected_counts().0 > 0).unwrap_or(false);
        let mut changed = false;
        if !p.extend.value && any_selected {
            common::deselect_all(cx.bmesh_mut(me)?);
            changed = true;
        }
        if !ungrouped.is_empty() {
            let bm = cx.bmesh_mut(me)?;
            for v in ungrouped {
                bm.vert_select_set(v, true);
            }
            changed = true;
        }
        if changed {
            common::selectmode_flush(cx.bmesh_mut(me)?);
        }
    }
    Ok(OpStatus::Finished)
}
