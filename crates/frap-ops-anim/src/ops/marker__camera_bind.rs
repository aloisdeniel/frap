// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.camera_bind()`, tier T0 (WP 7.6). Port of `ed_marker_camera_bind_exec`
//! (blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later: binds the **active object** (of any type: Blender does not check it is a
//! camera) to the marker on the current frame (the nearest marker, when it is on that frame),
//! else to a new selected marker `F_<frame>` appended at the current frame (every other marker
//! deselected); then the scene camera follows the markers (`BKE_scene_camera_switch_update`).
//! No active object: `Select a camera to bind to a marker on this frame` (`RuntimeError`).
//! Verified on 5.2.4: binding `Cube` to the marker at the current frame, then `Camera` at
//! another frame, makes `Camera` the scene camera.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::TimelineMarker;
use frap_spec_ops_anim::marker;

use super::marker__add::common;

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, _p: marker::CameraBind) -> OpResult {
    let (ob, cfra) = {
        let c = cx.ctx();
        (c.active_object(), c.scene().frame_current)
    };
    let Some(ob) = ob else {
        return Err(OpError::Runtime(
            "Select a camera to bind to a marker on this frame".into(),
        ));
    };
    common::with_markers(cx, |ms| {
        let at = common::nearest(ms, cfra as f32).filter(|&i| ms[i].frame == cfra);
        let i = match at {
            Some(i) => i,
            None => {
                common::deselect_all(ms);
                ms.push(TimelineMarker::new(&TimelineMarker::frame_name(cfra), cfra));
                ms.len() - 1
            }
        };
        ms[i].camera = Some(ob);
    });
    common::camera_switch(cx);
    Ok(OpStatus::Finished)
}
