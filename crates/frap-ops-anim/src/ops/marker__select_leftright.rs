// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.select_leftright(mode='LEFT', extend=False)`, tier T0 (WP 7.6). Port of
//! `ed_marker_select_leftright_exec` (blender/source/blender/editors/animation/
//! anim_markers.cc), Blender 5.2.4, GPL-2.0-or-later: without `extend` every marker is
//! deselected, then `LEFT` selects the markers on or before the current frame, `RIGHT` those on
//! or after it. `CLICK_SIDE` is resolved from the mouse in `invoke` only: in `exec` it selects
//! nothing (verified on 5.2.4: it only deselects). Poll `ed_markers_poll_markers_exist`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::marker::{self, select_leftright::Mode};

use super::marker__add::common;

#[operator(
    poll = common::EXIST,
    tier = T0,
    spec = marker::OPSPEC_MARKER_OT_SELECT_LEFTRIGHT,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, p: marker::MarkerSelectLeftright) -> OpResult {
    // `BKE_scene_frame_get`: frap stores no sub-frame.
    let cfra = cx.ctx().scene().frame_current as f32;
    let (mode, extend) = (p.mode.value, p.extend.value);
    common::with_markers(cx, |ms| {
        if !extend {
            common::deselect_all(ms);
        }
        for m in ms.iter_mut() {
            let f = m.frame as f32;
            if (mode == Mode::Left && f <= cfra) || (mode == Mode::Right && f >= cfra) {
                m.select = true;
            }
        }
    });
    Ok(OpStatus::Finished)
}
