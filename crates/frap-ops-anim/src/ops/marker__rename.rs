// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.rename(name='RenamedMarker')`, tier T0 (WP 7.6). Port of
//! `ed_marker_rename_exec` (blender/source/blender/editors/animation/anim_markers.cc),
//! Blender 5.2.4, GPL-2.0-or-later: the **first** selected marker gets `name` (cut to 63
//! bytes; not made unique). Poll `ed_markers_poll_selected_no_locked_markers`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::timeline_marker::NAME_MAX_BYTES;
use frap_spec_ops_anim::marker;
use smol_str::SmolStr;

use super::marker__add::common;

#[operator(
    poll = common::SELECTED_UNLOCKED_RENAME,
    tier = T0,
    support = HeadlessExtended(common::CONTEXT_NOTE)
)]
fn exec(cx: &mut OpCtx, p: marker::Rename) -> OpResult {
    let name = SmolStr::new(frap_rna::naming::truncate_utf8(
        &p.name.value,
        NAME_MAX_BYTES,
    ));
    let renamed = common::with_markers(cx, |ms| match ms.iter_mut().find(|m| m.select) {
        Some(m) => {
            m.name = name;
            true
        }
        None => false,
    })
    .unwrap_or(false);
    Ok(if renamed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
