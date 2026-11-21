// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.move(frames=0, tweak=False)`, tier T0 (WP 7.6). Port of
//! `ed_marker_move_exec` (`ed_marker_move_init`, `ed_marker_move_apply`;
//! blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later: every selected marker moves by `frames`, then the scene camera follows the
//! markers at the current frame (`BKE_scene_camera_switch_update`). `tweak` only matters to
//! `invoke`. Poll `ed_markers_poll_selected_no_locked_markers`.
//!
//! [`apply`] is shared with `marker.duplicate`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::marker;

use super::marker__add::common;

/// `ed_marker_move_exec`: moves the selected markers by `frames` and switches the camera;
/// `false` (Blender's `PASS_THROUGH`) when no marker is selected.
pub(crate) fn apply(cx: &mut OpCtx, frames: i32) -> bool {
    let moved = common::with_markers(cx, |ms| {
        let mut any = false;
        for m in ms.iter_mut().filter(|m| m.select) {
            m.frame = m.frame.wrapping_add(frames);
            any = true;
        }
        any
    })
    .unwrap_or(false);
    if moved {
        common::camera_switch(cx);
    }
    moved
}

#[operator(
    poll = common::SELECTED_UNLOCKED_MOVE,
    tier = T0,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, p: marker::Move) -> OpResult {
    // No selected marker: `PASS_THROUGH`, answered as `FINISHED` (the poll needs one anyway).
    apply(cx, p.frames.value);
    Ok(OpStatus::Finished)
}
