// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.nla.actionclip_add(action)`, tier T0 (WP 8.3.11), headless-extended. Port of
//! `nlaedit_add_actionclip_exec` (blender/source/blender/editors/space_nla/nla_edit.cc) with
//! `nlaedit_add_tracks_empty` (nla_tracks.cc), `BKE_nlastrip_new`, `BKE_nlatrack_add_strip`,
//! `BKE_nlastrip_validate_name` and `BKE_nla_validate_state` (blenkernel/intern/nla.cc),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Blender adds the strip to the active tracks listed in an NLA editor; frap has no editors, so
//! the **active object's** AnimData stands for the listed (and selected) block, as for
//! `nla.tracks_add` (OPERATORS.md, deviations §2). `action` is one of `bpy.data.actions` by
//! name (`RNA_action_itemf`); left unset it is the first action, as Blender's enum index 0, and
//! without any action the operator fails with `No valid action to add`. An AnimData without
//! tracks first gets one (`nlaedit_add_tracks_empty`; Blender does that only for an AnimData
//! selected in the NLA editor, which a fresh one is not, deviation D-164). Then each active, unlocked track gets an
//! action-clip strip of `action` (its slot auto-assigned for the object, the slot's key range or
//! the action's manual range) moved to start at `scene.frame_current`; a track without room
//! for it gets the strip on a new track at the top of the stack, which becomes active. The
//! strip is named after the action, made unique among the AnimData's strips; blends are
//! refitted (`BKE_nla_validate_state`). `FINISHED`.

use frap_op::EnumItemOwned;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::Root;
use frap_scene::Context;
use frap_spec_ops_anim::nla;
use smol_str::SmolStr;

use crate::keying::{adt_slot_mut, id_code_name, nla as nla_edit};

/// `RNA_action_itemf`: every action of `bpy.data.actions`, by name.
fn action_items(c: &Context) -> Vec<EnumItemOwned> {
    c.main
        .actions
        .iter()
        .map(|(_, a)| EnumItemOwned {
            identifier: a.header.name.clone(),
            name: a.header.name.clone(),
            description: SmolStr::default(),
        })
        .collect()
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    support = HeadlessExtended("the active object's AnimData stands for the NLA editor's channel list"),
    dyn_enums = [("action", action_items)]
)]
fn exec(cx: &mut OpCtx, p: nla::ActionclipAdd) -> OpResult {
    let cfra = cx.ctx().scene().frame_current as f32;
    let act = {
        let actions = &cx.main().actions;
        if p.action.value.is_empty() {
            actions.iter().next().map(|(r, _)| r)
        } else {
            actions
                .iter()
                .find(|(_, a)| a.header.name == p.action.value)
                .map(|(r, _)| r)
        }
    };
    let Some(act) = act else {
        return Err(OpError::Runtime("No valid action to add".into()));
    };
    let Some(ob) = cx.ctx().active_object() else {
        return Ok(OpStatus::Finished);
    };
    let root = Root::Id(ob.any());
    let Some(mut adt) = cx
        .main()
        .objects
        .get(ob)
        .and_then(|o| o.animation_data.clone())
    else {
        return Ok(OpStatus::Finished);
    };
    // `nlaedit_add_tracks_empty`
    if adt.nla_tracks.is_empty() {
        let t = nla_edit::track_new_tail(&mut adt);
        nla_edit::track_set_active(&mut adt, t);
    }
    // `nlaedit_get_editable_tracks`: the active, editable tracks.
    let tracks: Vec<usize> = (0..adt.nla_tracks.len())
        .filter(|&t| adt.nla_tracks[t].active && !adt.nla_tracks[t].lock)
        .collect();
    let id_name = id_code_name(cx.main(), root);
    let action_name = cx
        .main()
        .actions
        .get(act)
        .map(|a| a.header.name.to_string());
    for t in tracks {
        let mut strip = {
            let (main, log) = cx.main_mut();
            let Some(a) = main.actions.get_mut(act, log) else {
                break;
            };
            nla_edit::strip_new(act, a, "OBJECT", &id_name, None)
        };
        strip.frame_end += cfra - strip.frame_start;
        strip.frame_start = cfra;
        let (t, s) = match nla_edit::add_strip(&mut adt.nla_tracks[t], strip) {
            Ok(s) => (t, s),
            Err(strip) => {
                let nt = nla_edit::track_new_tail(&mut adt);
                nla_edit::track_set_active(&mut adt, nt);
                match nla_edit::add_strip(&mut adt.nla_tracks[nt], strip) {
                    Ok(s) => (nt, s),
                    Err(_) => continue,
                }
            }
        };
        nla_edit::validate_name(&mut adt, t, s, action_name.as_deref());
    }
    frap_anim::rna::anim_data::validate_state(&mut adt, None);
    let (main, log) = cx.main_mut();
    if let Some(slot) = adt_slot_mut(main, log, root) {
        *slot = Some(adt);
    }
    Ok(OpStatus::Finished)
}
