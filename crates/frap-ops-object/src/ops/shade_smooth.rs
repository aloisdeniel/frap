// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shade_smooth(keep_sharp_edges=True)`, tier T1
//! (`tests/golden/cases/object/shade/*.py`). Port of `shade_smooth_exec` with
//! `OBJECT_OT_shade_smooth` (`editors/object/object_edit.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! the meshes of the selected objects lose `sharp_face` (and `sharp_edge` with
//! `keep_sharp_edges=False`); see [`crate::support::shade`] for the verified rules and the poll.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use crate::support::shade::{self, SHADE_POLL, Shade};

#[operator(poll = SHADE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ShadeSmooth) -> OpResult {
    shade::run(cx, Shade::Smooth, p.keep_sharp_edges.value)
}
