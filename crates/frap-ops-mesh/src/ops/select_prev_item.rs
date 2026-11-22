// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_prev_item()`, tier T1 (`tests/golden/cases/edit/select_m8/
//! next_prev.py`). Port of `MeshSelectPrev` (`scripts/startup/bl_operators/mesh.py`) and
//! `find_adjacent.select_prev` (`scripts/startup/bl_operators/bmesh/find_adjacent.py`),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! On the active object's edit mesh: the newest selection-history entry is deselected (when
//! the entry before it is a face, that face becomes the active face) and the selection is
//! flushed, which drops the deselected entry from the history. With an empty history the INFO
//! `Last selected not found` is reported and nothing changes. Always finishes.

use frap_bmesh::BMElem;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::MeshId;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::SelectPrevItem) -> OpResult {
    let me = {
        let c = cx.ctx();
        poll::active(&c).and_then(|(_, o)| o.data?.typed::<MeshId>())
    };
    let Some(me) = me else {
        return Ok(OpStatus::Finished);
    };
    let Some(history) = common::read(cx, me, |bm| bm.select_history.clone()) else {
        return Ok(OpStatus::Finished);
    };
    let Some(&last) = history.last() else {
        cx.report(ReportType::Info, "Last selected not found");
        return Ok(OpStatus::Finished);
    };
    let bm = cx.bmesh_mut(me)?;
    bm.select_set(last, false);
    if history.len() >= 2
        && let BMElem::Face(f) = history[history.len() - 2]
    {
        bm.act_face = Some(f);
    }
    bm.select_flush_mode();
    Ok(OpStatus::Finished)
}
