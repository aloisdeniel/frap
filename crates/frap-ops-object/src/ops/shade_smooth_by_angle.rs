// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shade_smooth_by_angle(angle=0.523599, keep_sharp_edges=True)`, tier T1
//! (`tests/golden/cases/object/shade/*.py`). Port of `shade_smooth_exec` with
//! `OBJECT_OT_shade_smooth_by_angle` (`editors/object/object_edit.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): destructive smooth-by-angle, the meshes of the selected objects lose
//! `sharp_face` and get `sharp_edge` on the edges whose faces meet at more than `angle`; see
//! [`crate::support::shade`].

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use crate::support::shade::{self, SHADE_POLL, Shade};

#[operator(poll = SHADE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ShadeSmoothByAngle) -> OpResult {
    shade::run(cx, Shade::ByAngle(p.angle.value), p.keep_sharp_edges.value)
}
