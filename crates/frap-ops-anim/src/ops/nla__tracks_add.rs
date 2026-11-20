// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.nla.tracks_add(above_selected=False)`, tier T0 (WP 6.4), headless-extended. Port of
//! `nlaedit_add_tracks_exec` (`nlaedit_add_tracks_existing`, `nlaedit_add_tracks_empty`;
//! blender/source/blender/editors/space_nla/nla_tracks.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Blender works on the tracks and AnimData blocks listed (and selected) in an NLA editor;
//! frap has no editors, so the **active object's** AnimData stands for the listed block:
//! with `above_selected` a new track goes above every selected track, otherwise one new track
//! goes on top of the stack when a track is selected; an AnimData without tracks gets one.
//! Each new track (`NlaTrack`, `NlaTrack.001`, …) becomes the active one. Nothing added:
//! WARNING "Select an existing NLA Track or an empty action line first" and CANCELLED.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::Root;
use frap_spec_ops_anim::nla;

use crate::keying::{adt_slot_mut, nla as nla_edit};

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: nla::TracksAdd) -> OpResult {
    let ob = cx.ctx().active_object();
    let mut added = false;
    if let Some(ob) = ob {
        let (main, log) = cx.main_mut();
        if let Some(Some(adt)) = adt_slot_mut(main, log, Root::Id(ob.any())) {
            // `nlaedit_add_tracks_existing`
            let selected: Vec<usize> = (0..adt.nla_tracks.len())
                .filter(|&t| adt.nla_tracks[t].select)
                .collect();
            if p.above_selected.value {
                // Each insertion shifts the later selected tracks up by one.
                for (n, t) in selected.iter().enumerate() {
                    let new = nla_edit::track_new_after(adt, Some(t + n));
                    nla_edit::track_set_active(adt, new);
                    added = true;
                }
            } else if !selected.is_empty() {
                let new = nla_edit::track_new_tail(adt);
                nla_edit::track_set_active(adt, new);
                added = true;
            }
            // `nlaedit_add_tracks_empty`
            if adt.nla_tracks.is_empty() {
                let new = nla_edit::track_new_tail(adt);
                nla_edit::track_set_active(adt, new);
                added = true;
            }
        }
    }
    if !added {
        cx.report(
            ReportType::Warning,
            "Select an existing NLA Track or an empty action line first",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
