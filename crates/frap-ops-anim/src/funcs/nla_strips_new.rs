// SPDX-License-Identifier: GPL-3.0-or-later
//! `NlaTrack.strips.new(name, start, action) -> NlaStrip` (`rna_NlaStrip_new`,
//! blender/source/blender/makesrna/intern/rna_nla.cc, Blender 5.2.4): an action-clip strip
//! (`BKE_nlastrip_new`: the action's slot auto-assigned for the animated ID, the length of
//! the slot's keys or the manual frame range) moved to start at `start`, added to the track
//! when it fits (`RuntimeError: Error: Unable to add strip (the track does not have any space
//! to accommodate this new strip)` otherwise), named `name` made unique among the AnimData's
//! strips.

use frap_anim::rna::anim_data::AdtLoc;
use frap_rna::{ActionId, IdRef, RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::nla_tracks_new::adt;
use crate::keying::{id_code_name, nla};

#[rna_func(srna = "NlaStrips", func = "new")]
fn nla_strips_new(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let name = args
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("NlaStrip")
        .to_owned();
    let start = match args.get("start") {
        Some(Value::Int(i)) => *i as f32,
        _ => 0.0,
    };
    let action: Option<IdRef<ActionId>> = match args.get("action") {
        Some(Value::Id(Some(a))) => a.typed(),
        _ => None,
    };
    let Some(a) = action else {
        return Err(adt::error("Unable to create new strip"));
    };
    let Some((AdtLoc::Track(t), Some(_))) = adt::loc_of(h, this) else {
        return Err(adt::removed("NlaTrack"));
    };
    let id_type = match this.root {
        frap_rna::Root::Id(id) => frap_anim::action::id_type_of_prefix(id.code.as_str()),
        _ => "NODETREE",
    };
    let id_name = id_code_name(h.main(), this.root);
    let mut strip = {
        let (main, log) = h.main_mut();
        let act = main
            .actions
            .get_mut(a, log)
            .ok_or_else(|| adt::removed("Action"))?;
        nla::strip_new(a, act, id_type, &id_name, None)
    };
    let action_name = h
        .main()
        .actions
        .get(a)
        .map(|x| x.header.name.to_string());
    strip.frame_end += start - strip.frame_start;
    strip.frame_start = start;
    strip.name = name.into();
    let placed = adt::with_adt(h, this, |adt| {
        let track = adt
            .nla_tracks
            .get_mut(t)
            .ok_or_else(|| adt::removed("NlaTrack"))?;
        // `rna_NlaStrip_new` uses `BKE_nlastrips_add_strip` (no lock check).
        if !nla::has_space(&track.strips, strip.frame_start, strip.frame_end) {
            return Ok(None);
        }
        let at = track
            .strips
            .iter()
            .position(|s| s.frame_start >= strip.frame_end)
            .unwrap_or(track.strips.len());
        track.strips.insert(at, strip);
        nla::validate_name(adt, t, at, action_name.as_deref());
        Ok(Some(adt.nla_tracks[t].strips[at].name.clone()))
    })?;
    let Some(name) = placed else {
        return Err(adt::error(
            "Unable to add strip (the track does not have any space to accommodate this new strip)",
        ));
    };
    Ok(Value::Ptr(Box::new(
        this.child(Seg::Key(name), &frap_spec_rna::NLA_STRIP),
    )))
}
