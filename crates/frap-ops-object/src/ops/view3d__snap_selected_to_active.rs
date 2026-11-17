// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.snap_selected_to_active()`, tier T0
//! (`tests/golden/cases/object/snap/*.py`). Port of `snap_selected_to_active_exec`
//! (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4, GPL-2.0-or-later): every selected
//! editable object goes to the active object's origin; no active object →
//! `RuntimeError: Error: No active element found!` (verified).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::view3d;

use crate::support::snap;
use crate::support::xform::Worlds;

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = Partial("Object Mode only (Edit Mode M2, Pose Mode M6)")
)]
fn exec(cx: &mut OpCtx, _p: view3d::SnapSelectedToActive) -> OpResult {
    let target = {
        let c = cx.ctx();
        snap::object_mode_only(&c, "view3d.snap_selected_to_active")?;
        snap::active_center(&c, &Worlds::of(c.main), false)
    };
    let Some(target) = target else {
        return Err(OpError::Runtime("No active element found!".into()));
    };
    snap::selected_to_location(cx, target, false, None)
}
