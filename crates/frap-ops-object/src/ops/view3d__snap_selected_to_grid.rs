// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.snap_selected_to_grid()`, tier T0 (`tests/golden/cases/object/snap/*.py`).
//! Port of `snap_sel_to_grid_exec` (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): each selected editable object's origin is rounded to the nearest grid
//! line (see [`crate::support::cursor::grid_scale`]) through its parent space; children of
//! selected parents are snapped too, from their world location before the operator (verified).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::view3d;

use crate::support::{cursor, snap};

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = Partial("Object Mode only (Edit Mode M2, Pose Mode M6)")
)]
fn exec(cx: &mut OpCtx, _p: view3d::SnapSelectedToGrid) -> OpResult {
    let gridf = {
        let c = cx.ctx();
        snap::object_mode_only(&c, "view3d.snap_selected_to_grid")?;
        cursor::grid_scale(&c.scene().unit_settings)
    };
    snap::selected_to_grid(cx, gridf)
}
