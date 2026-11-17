// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.rotation_clear(clear_delta=False)`, tier T0
//! (`tests/golden/cases/object/apply/clear_*.py`). Port of `object_rotation_clear_exec` /
//! `object_clear_rot` (`editors/object/object_transform.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! the rotation of the object's current `rotation_mode` is reset (Euler `(0, 0, 0)`, quaternion
//! `(1, 0, 0, 0)`, axis-angle angle 0 about Y, `unit_axis_angle`); with `clear_delta` the
//! matching delta rotation too. The channels of the other modes are kept.

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
fn exec(cx: &mut OpCtx, p: object::RotationClear) -> OpResult {
    let delta = p.clear_delta.value;
    clear::run(cx, "rotation_clear", |o| match o.rotation_mode.as_str() {
        "QUATERNION" => {
            o.rotation_quaternion = [1.0, 0.0, 0.0, 0.0];
            if delta {
                o.delta_rotation_quaternion = [1.0, 0.0, 0.0, 0.0];
            }
        }
        // RNA exposes no delta axis-angle (DNA's is cleared, invisible).
        "AXIS_ANGLE" => o.rotation_axis_angle = [0.0, 0.0, 1.0, 0.0],
        _ => {
            o.rotation_euler = [0.0; 3];
            if delta {
                o.delta_rotation_euler = [0.0; 3];
            }
        }
    })
}
