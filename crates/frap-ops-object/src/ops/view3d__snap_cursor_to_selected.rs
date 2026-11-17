// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.snap_cursor_to_selected()`, tier T0
//! (`tests/golden/cases/object/snap/*.py`). Port of `snap_curs_to_sel_exec`
//! (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4, GPL-2.0-or-later): in Object Mode
//! the cursor goes to the median of the selected visible objects' origins (the middle of
//! their bounds with the `BOUNDING_BOX_CENTER` pivot); nothing selected → `CANCELLED`.

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
fn exec(cx: &mut OpCtx, _p: view3d::SnapCursorToSelected) -> OpResult {
    let (scene, center) = {
        let c = cx.ctx();
        snap::object_mode_only(&c, "view3d.snap_cursor_to_selected")?;
        (c.win.scene, snap::selection_center(&c, &Worlds::of(c.main)))
    };
    let Some(center) = center else {
        cx.report(
            ReportType::Warning,
            "frap: nothing to snap to: no visible object is selected",
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
