// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.snap_cursor_to_grid()`, tier T0 (`tests/golden/cases/object/snap/*.py`).
//! Port of `snap_curs_to_grid_exec` (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): each cursor coordinate is rounded to the nearest grid line of the
//! virtual Layout view (see [`crate::support::cursor::grid_scale`]).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::view3d;

use crate::support::cursor;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: view3d::SnapCursorToGrid) -> OpResult {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        let gridf = cursor::grid_scale(&s.unit_settings);
        s.cursor.location = cursor::snap_to_grid(s.cursor.location, gridf);
        cursor::sync_matrix(&mut s.cursor);
    }
    Ok(OpStatus::Finished)
}
