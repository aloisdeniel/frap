// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.duplicate(frames=0)`, tier T0 (WP 7.6). Port of `ed_marker_duplicate_exec`
//! (`ed_marker_duplicate_apply`, then `ed_marker_move_exec`;
//! blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later: each selected marker is deselected and a selected copy (name, frame,
//! camera) is put at the **head** of the list (Blender's "FIXME: bad ordering!": duplicating
//! A, B, C gives C', B', A', A, B, C), then the copies move by `frames` and the camera follows
//! (verified on 5.2.4). Poll `ed_markers_poll_selected_no_locked_markers`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::marker;

use super::marker__add::common;

#[operator(
    poll = common::SELECTED_UNLOCKED_DUPLICATE,
    tier = T0,
    spec = marker::OPSPEC_MARKER_OT_DUPLICATE,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, p: marker::MarkerDuplicate) -> OpResult {
    common::with_markers(cx, |ms| {
        let mut copies = Vec::new();
        for m in ms.iter_mut().filter(|m| m.select) {
            m.select = false;
            let mut copy = m.clone();
            copy.select = true;
            copies.push(copy);
        }
        // `BLI_addhead` per copy, in list order: the last copy ends up first.
        copies.reverse();
        ms.splice(0..0, copies);
    });
    super::marker__move::apply(cx, p.frames.value);
    Ok(OpStatus::Finished)
}
