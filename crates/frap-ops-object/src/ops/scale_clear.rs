// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.scale_clear(clear_delta=False)`, tier T0
//! (`tests/golden/cases/object/apply/clear_*.py`). Port of `object_scale_clear_exec` /
//! `object_clear_scale` (`editors/object/object_transform.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `scale = (1, 1, 1)` on the selected editable objects, `delta_scale` too
//! with `clear_delta`.

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
fn exec(cx: &mut OpCtx, p: object::ScaleClear) -> OpResult {
    let delta = p.clear_delta.value;
    clear::run(cx, "scale_clear", |o| {
        o.scale = [1.0; 3];
        if delta {
            o.delta_scale = [1.0; 3];
        }
    })
}
