// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.location_clear(clear_delta=False)`, tier T0
//! (`tests/golden/cases/object/apply/clear_*.py`). Port of `object_location_clear_exec` /
//! `object_clear_loc` (`editors/object/object_transform.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! `location = (0, 0, 0)` on the selected editable objects, `delta_location` too with
//! `clear_delta`. Poll `ED_operator_scene_editable` (passes in every mode, as dumped).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use crate::support::clear;

#[operator(
    poll = poll::SCENE_EDITABLE,
    tier = T0,
    support = Partial(
        "tool_settings.use_transform_data_origin and use_transform_skip_children are not \
         implemented (NotImplementedError)"
    )
)]
fn exec(cx: &mut OpCtx, p: object::LocationClear) -> OpResult {
    let delta = p.clear_delta.value;
    clear::run(cx, "location_clear", |o| {
        o.location = [0.0; 3];
        if delta {
            o.delta_location = [0.0; 3];
        }
    })
}
