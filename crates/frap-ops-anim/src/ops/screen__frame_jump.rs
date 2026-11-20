// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.screen.frame_jump(end=False)`, tier T0 (WP 6.4). Port of `frame_jump_exec`
//! (blender/source/blender/editors/screen/screen_ops.cc), Blender 5.2.4, GPL-2.0-or-later:
//! jumps to the scene's `frame_start` (or `frame_end`), the playback range
//! (`BKE_scene_get_playback_range`; frap has no preview range). A time change: animation is
//! applied by the evaluation after the operator (§7.3). No playback runs headless, so the
//! animation-timer branch never applies.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::screen;

use super::screen__frame_offset::frame;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: screen::FrameJump) -> OpResult {
    let (start, end) = {
        let c = cx.ctx();
        let s = c.scene();
        (s.frame_start, s.frame_end)
    };
    frame::set(cx, if p.end.value { end } else { start });
    Ok(OpStatus::Finished)
}
