// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.instance_offset_from_object()`, tier T0
//! (`tests/golden/cases/object/transform_m8/instance_offset.py`, WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/object.py (`DupliOffsetFromObject`),
//! Blender 5.2.4, GPL-2.0-or-later: `context.collection.instance_offset` takes the active
//! object's evaluated world location. Poll `context.active_object is not None` (a hidden
//! active object passes).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

/// `DupliOffsetFromObject.poll`: `context.active_object is not None`.
fn has_active(c: &Context) -> Result<(), PollFail> {
    if c.active_object()
        .is_some_and(|a| c.main.objects.contains(a))
    {
        return Ok(());
    }
    let hint = match c
        .view_layer()
        .bases
        .first()
        .and_then(|b| c.main.objects.get(b.object))
    {
        Some(o) => format!(
            "bpy.context.view_layer.objects.active = bpy.data.objects[{}]",
            poll::py_name(&o.header.name)
        ),
        None => "add an object first, e.g. bpy.ops.mesh.primitive_cube_add()".into(),
    };
    Err(PollFail {
        message: None,
        unmet: smallvec::smallvec![PollReq::ActiveObject],
        hint: Some(hint),
    })
}

/// `DupliOffsetFromObject.poll`.
const HAS_ACTIVE: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: has_active,
};

#[operator(poll = HAS_ACTIVE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::InstanceOffsetFromObject) -> OpResult {
    let Some(active) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let world = cx.evaluated(active)?.matrix_world;
    super::instance_offset_from_cursor::set_offset(cx, world.w_axis.truncate().to_array());
    Ok(OpStatus::Finished)
}
