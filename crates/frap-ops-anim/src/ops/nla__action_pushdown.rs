// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.nla.action_pushdown(track_index=-1)`, tier T0 (WP 6.4), headless-extended. Port of
//! `nlatracks_pushdown_exec` (blender/source/blender/editors/space_nla/nla_tracks.cc) with
//! `BKE_nla_action_pushdown` (blenkernel/intern/nla.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Blender polls for an NLA editor and takes the AnimData of its panel context; frap has no
//! editors, so `track_index=-1` acts on the **active object's** AnimData (OPERATORS.md:
//! "headless-extended"). The assigned action becomes an action-clip strip on the top NLA track
//! (a new track named after the action when the top one has no room), the action is
//! un-assigned (its slot remembered in `last_slot_identifier`), the strip takes the action's
//! blend type, influence and extrapolation and becomes active. An explicit `track_index`
//! addresses the NLA editor's channel list, which frap does not have (`NotImplementedError`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::Root;
use frap_spec_ops_anim::nla;

use crate::keying::{adt_slot_mut, id_code_name, nla as nla_edit};

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: nla::ActionPushdown) -> OpResult {
    if p.track_index.value != -1 {
        return Err(OpError::NotImplemented(
            "frap: nla.action_pushdown(track_index=…) addresses the NLA editor's channel list; \
             use track_index=-1 (the active object's AnimData)"
                .into(),
        ));
    }
    let Some(ob) = cx.ctx().active_object() else {
        return Err(OpError::Runtime(
            "No active AnimData block to use (select a data-block expander first or set the \
             appropriate flags on an AnimData block)"
                .into(),
        ));
    };
    let root = Root::Id(ob.any());
    let Some(adt) = cx
        .main()
        .objects
        .get(ob)
        .and_then(|o| o.animation_data.clone())
    else {
        return Err(OpError::Runtime(
            "No active AnimData block to use (select a data-block expander first or set the \
             appropriate flags on an AnimData block)"
                .into(),
        ));
    };
    if adt.use_tweak_mode {
        cx.report(
            ReportType::Warning,
            "Cannot push down actions while tweaking a strip's action, exit tweak mode first",
        );
        return Ok(OpStatus::Cancelled);
    }
    let Some(a) = adt.action else {
        cx.report(ReportType::Warning, "No active action to push down");
        return Ok(OpStatus::Cancelled);
    };
    let id_name = id_code_name(cx.main(), root);
    let (main, log) = cx.main_mut();
    let mut adt = adt;
    let pushed = match main.actions.get_mut(a, log) {
        Some(act) => nla_edit::action_pushdown(&mut adt, a, act, "OBJECT", &id_name),
        None => false,
    };
    if !pushed {
        return Ok(OpStatus::Cancelled);
    }
    if let Some(slot) = adt_slot_mut(main, log, root) {
        *slot = Some(adt);
    }
    Ok(OpStatus::Finished)
}
