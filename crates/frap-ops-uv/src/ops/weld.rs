// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.weld()`, tier T1 (`tests/golden/cases/uv/edit_m8/weld_*.py`). Port of
//! `uv_weld_exec`, `uv_weld` and `uvedit_uv_align_weld` (`editors/uvedit/uvedit_ops.cc`),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. The selected UVs of visible faces of every mesh in Edit Mode
//! with a UV map move to the centre of their bounds (`ED_uvedit_center_multi` with
//! `V3D_AROUND_CENTER_BOUNDS`, over all the meshes); meshes without a selected vertex are
//! skipped. Always finishes.

pub(crate) mod common;

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::{Uv, UvSel, op};
use common::Around;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: uv::Weld) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let objects = common::objects(cx);
    // `ED_uvedit_center_multi(scene, objects, cent, 0)`: mode 0 is `V3D_AROUND_CENTER_BOUNDS`.
    let cent = common::center_multi(cx, &objects, ts, Around::Bounds).unwrap_or([0.0; 2]);
    for &(_, me) in &objects {
        if common::no_vert_selected(cx, me) {
            continue;
        }
        common::edit(cx, me, ts, |s| ((), align_weld(s, Tool::Weld, cent)))?;
    }
    Ok(OpStatus::Finished)
}

/// `eUVWeldAlign` (the tools `uvedit_uv_align_weld` handles).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    /// `UV_ALIGN_X`: the U coordinate.
    AlignX,
    /// `UV_ALIGN_Y`: the V coordinate.
    AlignY,
    /// `UV_WELD`: both.
    Weld,
}

/// `uvedit_uv_align_weld`.
pub(crate) fn align_weld(s: &mut UvSel, tool: Tool, cent: Uv) -> bool {
    let mut changed = false;
    common::foreach_uv(s, true, true, |luv| {
        if matches!(tool, Tool::AlignX | Tool::Weld) && luv[0] != cent[0] {
            luv[0] = cent[0];
            changed = true;
        }
        if matches!(tool, Tool::AlignY | Tool::Weld) && luv[1] != cent[1] {
            luv[1] = cent[1];
            changed = true;
        }
    });
    changed
}
