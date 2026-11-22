// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_next_item()`, tier T1 (`tests/golden/cases/edit/select_m8/
//! next_prev.py`). Port of `MeshSelectNext` (`scripts/startup/bl_operators/mesh.py`) and
//! `find_adjacent.select_next` (`scripts/startup/bl_operators/bmesh/find_adjacent.py`),
//! Blender 5.2.4, GPL-2.0-or-later; the search is in `find_adjacent.rs`.
//!
//! On the active object's edit mesh: from the last two selection-history entries (the newest
//! is the destination, the one before the source) the element that continues the step is the
//! one whose vertex depths (breadth first over edges, and over face corners) from the
//! destination best match the source's and differ most from it; ties go to the element
//! farthest from the source, then to the closest match of the surrounding face sizes. It is
//! selected, becomes the newest history entry (and the active face), and the selection is
//! flushed. Otherwise an INFO tells why (`Selection pair not found`, `No single next item
//! found`, `Next element is hidden`) and nothing changes. Always finishes.

pub(crate) mod find_adjacent;

use frap_bmesh::BMElem;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::MeshId;
use frap_spec_ops_mesh::mesh;

use self::find_adjacent::{Next, select_next};
use super::select_more::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::SelectNextItem) -> OpResult {
    let me = {
        let c = cx.ctx();
        poll::active(&c).and_then(|(_, o)| o.data?.typed::<MeshId>())
    };
    let Some(me) = me else {
        return Ok(OpStatus::Finished);
    };
    let Some(next) = common::read(cx, me, select_next) else {
        return Ok(OpStatus::Finished);
    };
    match next {
        Next::Info(msg) => cx.report(ReportType::Info, msg),
        Next::Select(ele) => {
            let bm = cx.bmesh_mut(me)?;
            bm.select_set(ele, false);
            bm.select_set(ele, true);
            bm.select_history_remove(ele);
            bm.select_history_store_if_absent(ele);
            if let BMElem::Face(f) = ele {
                bm.act_face = Some(f);
            }
            bm.select_flush_mode();
        }
    }
    Ok(OpStatus::Finished)
}
