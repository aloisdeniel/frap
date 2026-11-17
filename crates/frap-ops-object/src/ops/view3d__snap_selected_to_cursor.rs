// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.snap_selected_to_cursor(use_offset=True, use_rotation=False)`, tier T0
//! (`tests/golden/cases/object/snap/*.py`). Port of `snap_selected_to_cursor_exec`
//! (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4, GPL-2.0-or-later): see
//! [`crate::support::snap`] for the verified rules. Nothing selected → `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::view3d;
use glam::Vec3;

use crate::support::{cursor, snap};

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = Partial("Object Mode only (Edit Mode M2, Pose Mode M6)")
)]
fn exec(cx: &mut OpCtx, p: view3d::SnapSelectedToCursor) -> OpResult {
    let (target, rot) = {
        let c = cx.ctx();
        snap::object_mode_only(&c, "view3d.snap_selected_to_cursor")?;
        let ts = c.tool_settings();
        if ts.use_transform_data_origin || ts.use_transform_skip_children {
            return Err(OpError::NotImplemented(
                "frap: snapping with tool_settings.use_transform_data_origin or \
                 use_transform_skip_children is not implemented yet"
                    .into(),
            ));
        }
        let cur = c.cursor();
        (
            Vec3::from_array(cur.location),
            p.use_rotation.value.then(|| cursor::rot_mat3(cur)),
        )
    };
    snap::selected_to_location(cx, target, p.use_offset.value, rot)
}
