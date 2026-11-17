// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.snap_cursor_to_center()`, tier T0 (`tests/golden/cases/object/snap/*.py`).
//! Port of `snap_curs_to_center_exec` (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): the 3D cursor goes to the world origin and its rotation, in its current
//! `rotation_mode` only, to the identity (verified: a quaternion cursor keeps its Euler values).
//! Poll `ED_operator_view3d_active`: frap's virtual 3D View always passes it (D-10).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::view3d;
use glam::Mat3;

use crate::support::cursor;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: view3d::SnapCursorToCenter) -> OpResult {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.cursor.location = [0.0; 3];
        cursor::set_rot_mat3(&mut s.cursor, &Mat3::IDENTITY, false);
    }
    Ok(OpStatus::Finished)
}
