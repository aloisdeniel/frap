// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.delete(confirm=True)`, tier T0 (WP 7.6). Port of `ed_marker_delete_exec`
//! (blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later: removes every selected marker; always `FINISHED`. `confirm` only asks in
//! `invoke` (scripts call `exec`). The scene camera stays as it is. Poll
//! `ed_markers_poll_selected_no_locked_markers` (`Markers are locked`, `No markers are
//! selected`; verified on 5.2.4).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::marker;

use super::marker__add::common;

#[operator(
    poll = common::SELECTED_UNLOCKED_DELETE,
    tier = T0,
    spec = marker::OPSPEC_MARKER_OT_DELETE,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, _p: marker::MarkerDelete) -> OpResult {
    common::with_markers(cx, |ms| ms.retain(|m| !m.select));
    Ok(OpStatus::Finished)
}
