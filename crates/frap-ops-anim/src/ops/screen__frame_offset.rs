// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.screen.frame_offset(delta=0)`, tier T0 (WP 6.4). Port of `frame_offset_exec`
//! (blender/source/blender/editors/screen/screen_ops.cc), Blender 5.2.4, GPL-2.0-or-later:
//! `frame_current += delta`, clamped at 0 (`FRAMENUMBER_MIN_CLAMP` with the factory preference
//! "Allow Negative Frames" off). A time change: animation is applied by the evaluation after
//! the operator (`bpy.ops` updates the view layer after a FINISHED call, §7.3). frap stores no
//! sub-frame, so the sub-frame correction of a negative delta never applies.
//!
//! [`frame`] holds the frame setter the `screen.*` operators share.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::screen;

/// Setting the context scene's frame from an operator.
pub(crate) mod frame {
    use frap_op::prelude::*;

    /// The context scene's `frame_current`.
    pub(crate) fn current(cx: &OpCtx) -> i32 {
        cx.ctx().scene().frame_current
    }

    /// Sets the context scene's `frame_current` and marks time dirty (`DEG_id_tag_update(…,
    /// ID_RECALC_FRAME_CHANGE)`).
    pub(crate) fn set(cx: &mut OpCtx, frame: i32) {
        let scene = cx.ctx().win.scene;
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.frame_current = frame;
        }
        cx.mark_time_dirty();
    }
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: screen::FrameOffset) -> OpResult {
    let delta = p.delta.value;
    let mut f = frame::current(cx).wrapping_add(delta);
    // `FRAMENUMBER_MIN_CLAMP` (negative frames not allowed by the factory preferences).
    if f < 0 {
        f = 0;
    }
    frame::set(cx, f);
    Ok(OpStatus::Finished)
}
