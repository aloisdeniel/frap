// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.nla.tracks_delete()`, tier T0 (WP 8.3.11), headless-extended. Port of
//! `nlaedit_delete_tracks_exec` (blender/source/blender/editors/space_nla/nla_tracks.cc) with
//! `BKE_nlatrack_remove_and_free` (blenkernel/intern/nla.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Blender deletes the selected tracks listed in an NLA editor; frap has no editors, so the
//! **active object's** AnimData stands for the listed blocks, as for `nla.tracks_add` and
//! `nla.action_pushdown` (OPERATORS.md, deviations §2): every selected track (`NlaTrack.select`)
//! is removed with its strips. Always `FINISHED`, also when nothing was deleted.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::Root;
use frap_spec_ops_anim::nla;

use crate::keying::adt_slot_mut;

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = HeadlessExtended("the active object's AnimData stands for the NLA editor's channel list")
)]
fn exec(cx: &mut OpCtx, _p: nla::TracksDelete) -> OpResult {
    let Some(ob) = cx.ctx().active_object() else {
        return Ok(OpStatus::Finished);
    };
    let any = cx
        .main()
        .objects
        .get(ob)
        .and_then(|o| o.animation_data.as_ref())
        .is_some_and(|adt| adt.nla_tracks.iter().any(|t| t.select));
    if !any {
        return Ok(OpStatus::Finished);
    }
    let (main, log) = cx.main_mut();
    if let Some(Some(adt)) = adt_slot_mut(main, log, Root::Id(ob.any())) {
        adt.nla_tracks.retain(|t| !t.select);
    }
    Ok(OpStatus::Finished)
}
