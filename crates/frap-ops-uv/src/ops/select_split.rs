// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_split()`, tier T1 (`tests/golden/cases/uv/select/split*.py`). Port of
//! `uv_select_split_exec` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. On every mesh in Edit Mode with a UV map, the UV vertices and
//! edges of visible faces with a mixed UV selection are deselected, so only entirely selected
//! faces stay selected. With sync selection only the UV selection changes (Blender does not
//! flush it to the mesh). `SHARED_VERTEX` with sync selection is refused (error);
//! `CANCELLED` when nothing changed.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::{Sticky, UvSel, op};

/// One mesh: whether anything changed.
fn split(s: &mut UvSel) -> bool {
    if s.ts.sync {
        s.prepare_sync_select();
    } else {
        s.prepare_custom_data();
    }
    let mut changed = false;
    for f in s.faces() {
        if !s.face_visible(f) {
            continue;
        }
        let mut is_sel = false;
        let mut is_unsel = false;
        for l in s.face_loops(f) {
            let v = s.loop_vert_select_get(l);
            let e = s.loop_edge_select_get(l);
            if v || e {
                is_sel = true;
            }
            if !v || !e {
                is_unsel = true;
            }
            if is_sel && is_unsel {
                break;
            }
        }
        if is_sel && is_unsel {
            for l in s.face_loops(f) {
                s.loop_vert_select_set(l, false);
                s.loop_edge_select_set(l, false);
            }
            changed = true;
        }
    }
    changed
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: uv::SelectSplit) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    if ts.sync && ts.sticky == Sticky::Vert {
        return Err(OpError::Runtime(
            "Cannot split selection with \"Sync Select\" and \"Shared Vertex\" selection enabled"
                .into(),
        ));
    }
    let mut changed = false;
    for (_, me) in op::objects(cx) {
        changed |= op::edit(cx, me, ts, split)?.unwrap_or(false);
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
