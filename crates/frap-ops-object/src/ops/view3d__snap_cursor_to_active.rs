// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.snap_cursor_to_active()`, tier T0
//! (`tests/golden/cases/object/snap/*.py`). Port of `snap_curs_to_active_exec`
//! (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4, GPL-2.0-or-later): the cursor goes to
//! the active object's origin, selected or not, hidden or not (verified); no active object →
//! `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::view3d;

use crate::support::xform::Worlds;
use crate::support::{cursor, snap};

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = Partial("Object Mode only (Edit Mode M2, Pose Mode M6)")
)]
fn exec(cx: &mut OpCtx, _p: view3d::SnapCursorToActive) -> OpResult {
    let (scene, center) = {
        let c = cx.ctx();
        snap::object_mode_only(&c, "view3d.snap_cursor_to_active")?;
        (
            c.win.scene,
            snap::active_center(&c, &Worlds::of(c.main), false),
        )
    };
    let Some(center) = center else {
        cx.report(
            ReportType::Warning,
            "frap: nothing to snap to: no active object",
        );
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.cursor.location = center.to_array();
        cursor::sync_matrix(&mut s.cursor);
    }
    Ok(OpStatus::Finished)
}
