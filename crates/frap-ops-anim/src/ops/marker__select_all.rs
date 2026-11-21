// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.select_all(action='TOGGLE')`, tier T0 (WP 7.6). Port of
//! `ed_marker_select_all_exec` and `ED_markers_deselect_all`
//! (blender/source/blender/editors/animation/anim_markers.cc), Blender 5.2.4,
//! GPL-2.0-or-later: `TOGGLE` deselects when a marker is selected, else selects; `INVERT`
//! flips every marker. Always `FINISHED`. Poll `ed_markers_poll_markers_exist`: markers not
//! locked and at least one marker, else `Operator bpy.ops.marker.select_all.poll() failed,
//! context is incorrect` (verified on 5.2.4).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::marker::{self, select_all::Action};

use super::marker__add::common;

#[operator(
    poll = common::EXIST,
    tier = T0,
    spec = marker::OPSPEC_MARKER_OT_SELECT_ALL,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, p: marker::MarkerSelectAll) -> OpResult {
    common::with_markers(cx, |ms| {
        let action = match p.action.value {
            Action::Toggle if ms.iter().any(|m| m.select) => Action::Deselect,
            Action::Toggle => Action::Select,
            a => a,
        };
        for m in ms.iter_mut() {
            m.select = match action {
                Action::Select => true,
                Action::Deselect => false,
                Action::Invert | Action::Toggle => !m.select,
            };
        }
    });
    Ok(OpStatus::Finished)
}
